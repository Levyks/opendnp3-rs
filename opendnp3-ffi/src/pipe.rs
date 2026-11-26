//! Channel-based byte pipe (`FfiBytePipe`) for targets where async foreign callback
//! traits are not supported (e.g. Python `asyncio` in BoltFFI 0.32.x).
//!
//! 100% Tokio-free and socket-free: target languages pump raw bytes between their
//! own TCP/TLS/serial sockets and [`FfiBytePipe::push_rx`] / [`FfiBytePipe::next_tx`].

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use boltffi::export;
use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{AsyncStream, StreamError, UnboundedReceiver, UnboundedSender, unbounded};

// ---- Minimal single-thread std timer (zero Tokio, zero external deps) ----

struct TimerEntry {
    deadline: Instant,
    id: u64,
    fired: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl PartialEq for TimerEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.id == other.id
    }
}
impl Eq for TimerEntry {}
impl PartialOrd for TimerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TimerEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deadline
            .cmp(&other.deadline)
            .then_with(|| self.id.cmp(&other.id))
    }
}

struct TimerDriver {
    heap: Mutex<BinaryHeap<Reverse<TimerEntry>>>,
    cvar: Condvar,
    next_id: AtomicU64,
}

impl TimerDriver {
    fn global() -> &'static Self {
        static DRIVER: OnceLock<TimerDriver> = OnceLock::new();
        DRIVER.get_or_init(|| {
            let driver = TimerDriver {
                heap: Mutex::new(BinaryHeap::new()),
                cvar: Condvar::new(),
                next_id: AtomicU64::new(1),
            };
            // Spawn the single daemon timer thread lazily on first use.
            // SAFETY-free: we access the static via a second OnceLock below.
            driver
        })
    }

    fn ensure_thread() {
        static STARTED: OnceLock<()> = OnceLock::new();
        STARTED.get_or_init(|| {
            let _ = thread::Builder::new()
                .name("opendnp3-ffi-timer".into())
                .spawn(|| {
                    let driver = TimerDriver::global();
                    let mut heap = driver.heap.lock().unwrap();
                    loop {
                        let now = Instant::now();
                        while let Some(Reverse(top)) = heap.peek() {
                            if top.deadline <= now {
                                let Reverse(entry) = heap.pop().unwrap();
                                entry.fired.store(true, Ordering::Release);
                                if let Some(w) = entry.waker.lock().unwrap().take() {
                                    w.wake();
                                }
                            } else {
                                break;
                            }
                        }
                        if let Some(Reverse(top)) = heap.peek() {
                            let wait = top.deadline.saturating_duration_since(Instant::now());
                            if !wait.is_zero() {
                                let (g, _) = driver.cvar.wait_timeout(heap, wait).unwrap();
                                heap = g;
                            }
                        } else {
                            heap = driver.cvar.wait(heap).unwrap();
                        }
                    }
                });
        });
    }

    fn register(deadline: Instant, fired: Arc<AtomicBool>, waker: Arc<Mutex<Option<Waker>>>) {
        Self::ensure_thread();
        let driver = Self::global();
        let id = driver.next_id.fetch_add(1, Ordering::Relaxed);
        let mut heap = driver.heap.lock().unwrap();
        let should_notify = heap
            .peek()
            .is_none_or(|Reverse(top)| deadline < top.deadline);
        heap.push(Reverse(TimerEntry {
            deadline,
            id,
            fired,
            waker,
        }));
        if should_notify {
            driver.cvar.notify_one();
        }
    }
}

struct StdSleep {
    deadline: Instant,
    registered: bool,
    fired: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl StdSleep {
    fn new(duration: Duration) -> Self {
        Self {
            deadline: Instant::now() + duration,
            registered: false,
            fired: Arc::new(AtomicBool::new(false)),
            waker: Arc::new(Mutex::new(None)),
        }
    }
}

impl Future for StdSleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.fired.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            return Poll::Ready(());
        }
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        if !self.registered {
            self.registered = true;
            TimerDriver::register(
                self.deadline,
                Arc::clone(&self.fired),
                Arc::clone(&self.waker),
            );
        }
        if self.fired.load(Ordering::Acquire) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

// ---- Exported FfiBytePipe ----

/// Bidirectional byte pipe for driving [`FfiMaster::run_with_pipe`] or
/// [`FfiOutstation::run_with_pipe`] from any async runtime (such as Python `asyncio`).
///
/// The target language owns the TCP/TLS/serial socket:
/// - Push received socket bytes via [`push_rx`](Self::push_rx).
/// - Await outgoing DNP3 bytes via [`next_tx`](Self::next_tx) and write them to the socket.
/// - Call [`close`](Self::close) on EOF or socket error.
#[export]
pub struct FfiBytePipe {
    origin: Instant,
    rx_tx: Mutex<Option<UnboundedSender<Vec<u8>>>>,
    rx_rx: UnboundedReceiver<Vec<u8>>,
    tx_tx: Mutex<Option<UnboundedSender<Vec<u8>>>>,
    tx_rx: UnboundedReceiver<Vec<u8>>,
}

#[export]
impl FfiBytePipe {
    pub fn new() -> Self {
        let (rx_tx, rx_rx) = unbounded();
        let (tx_tx, tx_rx) = unbounded();
        Self {
            origin: Instant::now(),
            rx_tx: Mutex::new(Some(rx_tx)),
            rx_rx,
            tx_tx: Mutex::new(Some(tx_tx)),
            tx_rx,
        }
    }

    /// Push raw bytes received from the underlying socket/serial port into DNP3.
    /// Returns `false` if the pipe has been closed.
    pub fn push_rx(&self, data: Vec<u8>) -> bool {
        if data.is_empty() {
            return self.rx_tx.lock().unwrap().is_some();
        }
        if let Some(tx) = self.rx_tx.lock().unwrap().as_ref() {
            tx.send(data).is_ok()
        } else {
            false
        }
    }

    /// Await the next chunk of outgoing DNP3 bytes to write to the underlying socket.
    /// Returns `None` once the session exits or [`close`](Self::close) is called.
    pub async fn next_tx(&self) -> Option<Vec<u8>> {
        self.tx_rx.recv().await
    }

    /// Signal EOF / transport closure to both the session and any awaiting [`next_tx`](Self::next_tx).
    pub fn close_pipe(&self) {
        self.rx_tx.lock().unwrap().take();
        self.tx_tx.lock().unwrap().take();
    }
}

impl Default for FfiBytePipe {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct PipeStream<'a> {
    pipe: &'a FfiBytePipe,
}

impl<'a> PipeStream<'a> {
    pub(crate) fn new(pipe: &'a FfiBytePipe) -> Self {
        Self { pipe }
    }
}

impl Drop for PipeStream<'_> {
    fn drop(&mut self) {
        // Close tx so the target language's `next_tx()` pump exits cleanly.
        self.pipe.tx_tx.lock().unwrap().take();
    }
}

impl AsyncStream for PipeStream<'_> {
    async fn read(&self, _max_bytes: usize) -> Result<Vec<u8>, StreamError> {
        self.pipe.rx_rx.recv().await.ok_or(StreamError::Closed)
    }

    async fn write(&self, data: &[u8]) -> Result<(), StreamError> {
        let guard = self.pipe.tx_tx.lock().unwrap();
        match guard.as_ref() {
            Some(tx) => tx.send(data.to_vec()).map_err(|_| StreamError::Closed),
            None => Err(StreamError::Closed),
        }
    }

    async fn sleep(&self, duration: Duration) {
        StdSleep::new(duration).await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(self.pipe.origin.elapsed().as_millis() as u64)
    }
}
