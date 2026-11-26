//! Tokio-free async driver test proving `opendnp3::driver` runs end-to-end on
//! any minimal executor (such as Python/C#/Java polling Rust futures via BoltFFI).

#![cfg(feature = "async")]

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use futures_util::future::join3;
use opendnp3::app::{Analog, Binary, HeaderInfo, Indexed};
use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{
    AsyncStream, StreamError, UnboundedReceiver, UnboundedSender, master_channel,
    outstation_channel, run_master_session, run_outstation_session, unbounded,
};
use opendnp3::enums::{EventMode, MasterTaskType, TaskCompletion};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{DefaultMasterApplication, Master, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{
    DatabaseConfig, DefaultCommandHandler, DefaultOutstationApplication, Outstation,
    OutstationConfig,
};

struct ThreadWaker(Thread);
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Minimal 10-line std-only executor (zero Tokio!).
fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => thread::park(),
        }
    }
}

/// In-memory `AsyncStream` backed by our `no_std` channel (zero sockets, zero Tokio).
struct MemStream {
    origin: Instant,
    rx: UnboundedReceiver<Vec<u8>>,
    tx: UnboundedSender<Vec<u8>>,
}

fn mem_stream_pair() -> (MemStream, MemStream) {
    let origin = Instant::now();
    let (a_tx, b_rx) = unbounded();
    let (b_tx, a_rx) = unbounded();
    (
        MemStream {
            origin,
            rx: a_rx,
            tx: a_tx,
        },
        MemStream {
            origin,
            rx: b_rx,
            tx: b_tx,
        },
    )
}

impl AsyncStream for MemStream {
    async fn read(&self, _max_bytes: usize) -> Result<Vec<u8>, StreamError> {
        self.rx.recv().await.ok_or(StreamError::Closed)
    }

    async fn write(&self, data: &[u8]) -> Result<(), StreamError> {
        self.tx.send(data.to_vec()).map_err(|_| StreamError::Closed)
    }

    async fn sleep(&self, duration: Duration) {
        let (tx, rx) = unbounded();
        thread::spawn(move || {
            thread::sleep(duration);
            let _ = tx.send(());
        });
        let _ = rx.recv().await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(self.origin)
    }
}

#[derive(Default)]
struct RecordingSoe {
    binaries: Vec<Indexed<Binary>>,
    analogs: Vec<Indexed<Analog>>,
}

impl SoeHandler for RecordingSoe {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.extend_from_slice(values);
    }
    fn process_analog(&mut self, _info: HeaderInfo, values: &[Indexed<Analog>]) {
        self.analogs.extend_from_slice(values);
    }
}

#[test]
fn runtime_agnostic_async_master_and_outstation_without_tokio() {
    let (m_stream, o_stream) = mem_stream_pair();

    let mut m_link = LinkConfig::new(true);
    m_link.local_addr = 1;
    m_link.remote_addr = 10;
    let mut o_link = LinkConfig::new(false);
    o_link.local_addr = 10;
    o_link.remote_addr = 1;

    let mut master = Master::new(MasterParams::default(), Addresses::new(1, 10));
    let mut m_soe = RecordingSoe::default();
    let mut m_app = DefaultMasterApplication;
    let (m_handle, m_cmd_rx) = master_channel();
    let (ev_tx, ev_rx) = unbounded();

    let o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(3),
        ..Default::default()
    };
    let mut outstation = Outstation::new(o_cfg, Addresses::new(10, 1));
    outstation
        .database
        .update_binary(0, Binary::new(true), EventMode::Suppress);
    outstation
        .database
        .update_analog(1, Analog::new(77.0), EventMode::Suppress);
    let mut o_cmd = DefaultCommandHandler;
    let mut o_app = DefaultOutstationApplication;
    let (o_handle, o_cmd_rx) = outstation_channel();

    let m_task = run_master_session(
        &m_stream,
        m_link,
        &mut master,
        &mut m_soe,
        &mut m_app,
        &m_cmd_rx,
        &ev_tx,
    );
    let o_task = run_outstation_session(
        &o_stream,
        o_link,
        &mut outstation,
        &mut o_cmd,
        &mut o_app,
        &o_cmd_rx,
    );
    let observer = async move {
        while let Some(ev) = ev_rx.recv().await {
            if matches!(
                ev,
                MasterEvent::TaskComplete {
                    task_type: MasterTaskType::StartupIntegrityPoll,
                    result: TaskCompletion::Success,
                }
            ) {
                m_handle.shutdown();
                o_handle.shutdown();
                break;
            }
        }
    };

    let (m_shutdown, o_shutdown, ()) = block_on(join3(m_task, o_task, observer));
    assert!(m_shutdown);
    assert!(o_shutdown);
    assert!(m_soe.binaries.iter().any(|b| b.index == 0 && b.value.value));
    assert!(
        m_soe
            .analogs
            .iter()
            .any(|a| a.index == 1 && (a.value.value - 77.0).abs() < 1e-6)
    );
}
