//! `compio` completion-based (`io_uring` / IOCP) async adapter for [`opendnp3`].

use std::cell::RefCell;
use std::time::{Duration, Instant};

use compio::buf::BufResult;
use compio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use compio::net::TcpStream;
use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{
    LocalAsyncStream, MasterCommand, OutstationCommand, StreamError, UnboundedReceiver,
    UnboundedSender, run_master_session_local, run_outstation_session_local,
};
use opendnp3::link::LinkConfig;
use opendnp3::master::{Master, MasterApplication, MasterEvent, SoeHandler};
use opendnp3::outstation::{CommandHandler, Outstation, OutstationApplication};

/// [`LocalAsyncStream`] adapter for `compio` streams (`TcpStream`, etc.).
///
/// Uses `compio`'s completion-based owned-buffer I/O (`io_uring` on Linux,
/// IOCP on Windows).
pub struct CompioStream<S> {
    origin: Instant,
    stream: RefCell<S>,
}

impl<S> CompioStream<S> {
    pub fn new(stream: S) -> Self {
        Self {
            origin: Instant::now(),
            stream: RefCell::new(stream),
        }
    }
}

impl LocalAsyncStream for CompioStream<TcpStream> {
    async fn read(&self, max_bytes: usize) -> Result<Vec<u8>, StreamError> {
        let mut s = self.stream.borrow().clone();
        let buf = Vec::with_capacity(max_bytes);
        let BufResult(res, buf) = s.read(buf).await;
        match res {
            Ok(0) => Err(StreamError::Closed),
            Ok(_) => Ok(buf),
            Err(e) => Err(StreamError::Io(e.to_string())),
        }
    }

    async fn write(&self, data: &[u8]) -> Result<(), StreamError> {
        let mut s = self.stream.borrow().clone();
        let BufResult(res, _) = s.write_all(data.to_vec()).await;
        res.map_err(|e| StreamError::Io(e.to_string()))?;
        s.flush().await.map_err(|e| StreamError::Io(e.to_string()))
    }

    async fn sleep(&self, duration: Duration) {
        compio::time::sleep(duration).await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(self.origin)
    }
}

/// Drive a [`Master`] session over a `compio` [`TcpStream`].
pub async fn run_compio_master<H, A>(
    stream: TcpStream,
    link_cfg: LinkConfig,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<MasterCommand>,
    ev_tx: &UnboundedSender<MasterEvent>,
) -> bool
where
    H: SoeHandler,
    A: MasterApplication,
{
    let wrapped = CompioStream::new(stream);
    run_master_session_local(&wrapped, link_cfg, master, soe, app, cmd_rx, ev_tx).await
}

/// Drive an [`Outstation`] session over a `compio` [`TcpStream`].
pub async fn run_compio_outstation<C, A>(
    stream: TcpStream,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<OutstationCommand>,
) -> bool
where
    C: CommandHandler,
    A: OutstationApplication,
{
    let wrapped = CompioStream::new(stream);
    run_outstation_session_local(&wrapped, link_cfg, outstation, cmd_handler, app, cmd_rx).await
}
