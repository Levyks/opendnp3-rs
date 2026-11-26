//! `smol` runtime adapter for [`opendnp3`] built on [`opendnp3::driver::AsyncStream`].

use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{
    AsyncStream, MasterCommand, MasterHandle, OutstationCommand, OutstationHandle, StreamError,
    UnboundedReceiver, UnboundedSender, apply_outstation_command, master_channel,
    outstation_channel, run_master_session as drive_master,
    run_outstation_session as drive_outstation, unbounded,
};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{Master, MasterApplication, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{CommandHandler, Outstation, OutstationApplication, OutstationConfig};
use smol::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use smol::lock::Mutex;
use smol::net::{TcpListener, TcpStream};

/// [`AsyncStream`] wrapper for any `smol::io::AsyncRead + AsyncWrite`.
pub struct SmolStream<S> {
    origin: Instant,
    reader: Mutex<ReadHalf<S>>,
    writer: Mutex<WriteHalf<S>>,
}

impl<S> SmolStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    pub fn new(stream: S) -> Self {
        let (r, w) = smol::io::split(stream);
        Self {
            origin: Instant::now(),
            reader: Mutex::new(r),
            writer: Mutex::new(w),
        }
    }
}

impl<S> AsyncStream for SmolStream<S>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
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
        smol::Timer::after(duration).await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(self.origin)
    }
}

/// Drive a single connected `smol` stream for a [`Master`].
pub async fn run_master_session<S, H, A>(
    stream: S,
    link_cfg: LinkConfig,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<MasterCommand>,
    ev_tx: &UnboundedSender<MasterEvent>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    H: SoeHandler,
    A: MasterApplication,
{
    let wrapped = SmolStream::new(stream);
    drive_master(&wrapped, link_cfg, master, soe, app, cmd_rx, ev_tx).await
}

/// Drive a single connected `smol` stream for an [`Outstation`].
pub async fn run_outstation_session<S, C, A>(
    stream: S,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<OutstationCommand>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    C: CommandHandler,
    A: OutstationApplication,
{
    let wrapped = SmolStream::new(stream);
    drive_outstation(&wrapped, link_cfg, outstation, cmd_handler, app, cmd_rx).await
}

/// Spawn a TCP Master task on the `smol` executor.
pub async fn spawn_tcp_master<H, A>(
    addr: SocketAddr,
    link_cfg: LinkConfig,
    params: MasterParams,
    mut soe: H,
    mut app: A,
) -> io::Result<(MasterHandle, UnboundedReceiver<MasterEvent>)>
where
    H: SoeHandler + Send + 'static,
    A: MasterApplication + Send + 'static,
{
    let stream = TcpStream::connect(addr).await?;
    let _ = stream.set_nodelay(true);
    let (handle, cmd_rx) = master_channel();
    let (ev_tx, ev_rx) = unbounded();

    smol::spawn(async move {
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut master = Master::new(params, addresses);
        let _ = run_master_session(
            stream,
            link_cfg,
            &mut master,
            &mut soe,
            &mut app,
            &cmd_rx,
            &ev_tx,
        )
        .await;
    })
    .detach();

    Ok((handle, ev_rx))
}

/// Spawn a TCP Outstation task on the `smol` executor.
pub fn spawn_tcp_outstation<C, A>(
    listener: TcpListener,
    link_cfg: LinkConfig,
    outstation_cfg: OutstationConfig,
    mut cmd_handler: C,
    mut app: A,
) -> OutstationHandle
where
    C: CommandHandler + Send + 'static,
    A: OutstationApplication + Send + 'static,
{
    let (handle, rx) = outstation_channel();
    smol::spawn(async move {
        let origin = Instant::now();
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut outstation = Outstation::new(outstation_cfg, addresses);
        loop {
            let accept_or_cmd = smol::future::or(async { Ok(listener.accept().await) }, async {
                Err(rx.recv().await)
            })
            .await;
            match accept_or_cmd {
                Ok(Ok((stream, _peer))) => {
                    let _ = stream.set_nodelay(true);
                    let shutdown = run_outstation_session(
                        stream,
                        link_cfg.clone(),
                        &mut outstation,
                        &mut cmd_handler,
                        &mut app,
                        &rx,
                    )
                    .await;
                    if shutdown {
                        break;
                    }
                }
                Ok(Err(_)) => break,
                Err(Some(OutstationCommand::Shutdown)) | Err(None) => break,
                Err(Some(other)) => {
                    apply_outstation_command(
                        &mut outstation,
                        other,
                        MonotonicTime::from_elapsed(origin),
                        &app,
                    );
                }
            }
        }
    })
    .detach();
    handle
}
