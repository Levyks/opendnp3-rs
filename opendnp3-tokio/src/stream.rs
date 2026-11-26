//! [`opendnp3::driver::AsyncStream`] adapter for any Tokio `AsyncRead + AsyncWrite`.

use std::time::{Duration, Instant};

use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{AsyncStream, StreamError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::Mutex;

/// Wraps any Tokio `AsyncRead + AsyncWrite` stream (TCP, TLS, serial, duplex, ...)
/// into an [`AsyncStream`] so it can drive [`opendnp3::driver::run_master_session`]
/// or [`opendnp3::driver::run_outstation_session`].
pub struct TokioStream<S> {
    origin: Instant,
    reader: Mutex<ReadHalf<S>>,
    writer: Mutex<WriteHalf<S>>,
}

impl<S> TokioStream<S>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    pub fn new(stream: S) -> Self {
        let (r, w) = tokio::io::split(stream);
        Self {
            origin: Instant::now(),
            reader: Mutex::new(r),
            writer: Mutex::new(w),
        }
    }
}

impl<S> AsyncStream for TokioStream<S>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    async fn read(&self, max_bytes: usize) -> Result<Vec<u8>, StreamError> {
        let mut buf = vec![0u8; max_bytes];
        let mut reader = self.reader.lock().await;
        match reader.read(&mut buf).await {
            Ok(0) => Err(StreamError::Closed),
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(e) => Err(StreamError::Io(e.to_string())),
        }
    }

    async fn write(&self, data: &[u8]) -> Result<(), StreamError> {
        let mut writer = self.writer.lock().await;
        writer
            .write_all(data)
            .await
            .map_err(|e| StreamError::Io(e.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|e| StreamError::Io(e.to_string()))
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(self.origin)
    }
}
