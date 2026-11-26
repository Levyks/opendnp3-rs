//! Runtime-agnostic, `no_std + alloc` async byte-stream traits for driving DNP3
//! sessions on any executor (Tokio, smol, Embassy, compio, or BoltFFI).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::future::Future;
use core::time::Duration;

use crate::clock::MonotonicTime;

/// Error returned by an [`AsyncStream`] or [`LocalAsyncStream`] implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// The peer closed the connection cleanly (EOF).
    Closed,
    /// Underlying transport or I/O failure.
    Io(String),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => write!(f, "stream closed"),
            Self::Io(msg) => write!(f, "stream I/O error: {msg}"),
        }
    }
}

impl core::error::Error for StreamError {}

/// A single live bidirectional byte stream + async timer with `Send` futures
/// (for multi-threaded runtimes like Tokio, smol, and BoltFFI).
pub trait AsyncStream: Send + Sync {
    /// Read up to `max_bytes` from the stream.
    /// Returning `Ok(vec![])` or `Err(StreamError::Closed)` signals EOF.
    fn read(&self, max_bytes: usize) -> impl Future<Output = Result<Vec<u8>, StreamError>> + Send;

    /// Write all bytes in `data` and flush them to the underlying transport.
    fn write(&self, data: &[u8]) -> impl Future<Output = Result<(), StreamError>> + Send;

    /// Sleep asynchronously for `duration` using the host runtime's timer.
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send;

    /// Current monotonic timestamp in milliseconds.
    fn now(&self) -> MonotonicTime;
}

/// Single-threaded / `!Send` variant of [`AsyncStream`] for thread-per-core or
/// embedded executors (e.g. `compio` `io_uring`/IOCP and `embassy`).
#[allow(async_fn_in_trait)]
pub trait LocalAsyncStream {
    async fn read(&self, max_bytes: usize) -> Result<Vec<u8>, StreamError>;
    async fn write(&self, data: &[u8]) -> Result<(), StreamError>;
    async fn sleep(&self, duration: Duration);
    fn now(&self) -> MonotonicTime;
}
