//! Built-in Rust TCP transport powered by `smol`'s core (`async-net` + `async-io`).
//!
//! Zero per-packet FFI hops: TCP I/O, link framing, timers, and reconnect loops
//! stay entirely in Rust while BoltFFI polls the top-level future from the host
//! language's async runtime.

use std::future::Future;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use async_executor::{Executor, Task};
use async_io::Timer;
use async_net::{TcpListener as AsyncTcpListener, TcpStream as AsyncTcpStream};
use boltffi::{data, export};
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{AsyncStream, StreamError as CoreStreamError};

use crate::types::StreamError;

/// Execution mode for built-in Rust `smol-tcp` sessions.
#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpMode {
    /// Poll directly on the host/reactor wake path (lowest single-connection latency).
    Direct,
    /// Spawn onto the shared multi-threaded work-stealing `async-executor` pool.
    Pooled,
}

/// Shared multi-threaded work-stealing `smol` executor so concurrent TCP sessions
/// scale across all CPU cores instead of running on `async-io`'s single reactor thread.
pub(crate) fn spawn_on_pool<F, T>(fut: F) -> Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    static EXECUTOR: OnceLock<Executor<'static>> = OnceLock::new();
    static WORKERS: OnceLock<()> = OnceLock::new();

    let ex = EXECUTOR.get_or_init(Executor::new);
    WORKERS.get_or_init(|| {
        let count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        for i in 0..count {
            let _ = std::thread::Builder::new()
                .name(format!("opendnp3-smol-{i}"))
                .spawn(|| {
                    let ex = EXECUTOR.get().expect("executor initialized");
                    futures_lite::future::block_on(ex.run(std::future::pending::<()>()));
                });
        }
    });
    ex.spawn(fut)
}

/// Bound TCP listener for [`Outstation::run_tcp_server`] or [`Master::run_tcp_server`].
#[export]
pub struct TcpListener {
    inner: std::sync::Arc<AsyncTcpListener>,
    local_port: u16,
}

#[export]
impl TcpListener {
    /// Bind a TCP listener on `host:port` (pass `port = 0` to let the OS choose an ephemeral port).
    pub async fn bind(host: String, port: u16) -> Result<Self, StreamError> {
        let inner = AsyncTcpListener::bind((host.as_str(), port))
            .await
            .map_err(|e| StreamError::Io {
                message: e.to_string(),
            })?;
        let local_port = inner
            .local_addr()
            .map_err(|e| StreamError::Io {
                message: e.to_string(),
            })?
            .port();
        Ok(Self {
            inner: std::sync::Arc::new(inner),
            local_port,
        })
    }

    /// Return the local TCP port this listener is bound to.
    pub fn port(&self) -> u16 {
        self.local_port
    }
}

impl TcpListener {
    pub(crate) fn clone_inner(&self) -> std::sync::Arc<AsyncTcpListener> {
        std::sync::Arc::clone(&self.inner)
    }

    pub(crate) async fn accept_inner(
        inner: &AsyncTcpListener,
    ) -> Result<AsyncTcpStream, StreamError> {
        let (stream, _peer) = inner.accept().await.map_err(|e| StreamError::Io {
            message: e.to_string(),
        })?;
        let _ = stream.set_nodelay(true);
        Ok(stream)
    }
}

pub(crate) struct SmolTcpStream {
    origin: Instant,
    stream: AsyncTcpStream,
}

impl SmolTcpStream {
    pub(crate) fn new(stream: AsyncTcpStream) -> Self {
        let _ = stream.set_nodelay(true);
        Self {
            origin: Instant::now(),
            stream,
        }
    }

    pub(crate) async fn connect(host: &str, port: u16) -> Result<Self, StreamError> {
        let stream = AsyncTcpStream::connect((host, port))
            .await
            .map_err(|e| StreamError::Io {
                message: e.to_string(),
            })?;
        Ok(Self::new(stream))
    }
}

impl AsyncStream for SmolTcpStream {
    async fn read(&self, max_bytes: usize) -> Result<Vec<u8>, CoreStreamError> {
        let mut buf = vec![0u8; max_bytes];
        let mut reader = self.stream.clone();
        match reader.read(&mut buf).await {
            Ok(0) => Err(CoreStreamError::Closed),
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(e) => Err(CoreStreamError::Io(e.to_string())),
        }
    }

    async fn write(&self, data: &[u8]) -> Result<(), CoreStreamError> {
        let mut writer = self.stream.clone();
        writer
            .write_all(data)
            .await
            .map_err(|e| CoreStreamError::Io(e.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|e| CoreStreamError::Io(e.to_string()))
    }

    async fn sleep(&self, duration: Duration) {
        Timer::after(duration).await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(self.origin)
    }
}
