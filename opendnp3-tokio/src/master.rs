//! Tokio Master helpers built on top of [`opendnp3::driver::run_master_session`].

use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{
    MasterCommand, MasterHandle, UnboundedReceiver, UnboundedSender, apply_master_command,
    master_channel, run_master_session as drive_master_session, unbounded,
};
use opendnp3::enums::ChannelState;
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{Master, MasterApplication, MasterEvent, MasterParams, SoeHandler};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::stream::TokioStream;

/// Exponential backoff configuration for reconnecting TCP master channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryStrategy {
    pub min_retry_delay: Duration,
    pub max_retry_delay: Duration,
}

impl RetryStrategy {
    pub const fn new(min_retry_delay: Duration, max_retry_delay: Duration) -> Self {
        Self {
            min_retry_delay,
            max_retry_delay,
        }
    }

    pub fn next_delay(&self, current: Duration) -> Duration {
        current
            .saturating_mul(2)
            .clamp(self.min_retry_delay, self.max_retry_delay)
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            min_retry_delay: Duration::from_secs(1),
            max_retry_delay: Duration::from_secs(30),
        }
    }
}

/// Drive a single connected Tokio `AsyncRead + AsyncWrite` stream for a [`Master`].
/// Returns `true` if `Shutdown` was requested, `false` if the stream closed.
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
    S: AsyncRead + AsyncWrite + Send + 'static,
    H: SoeHandler,
    A: MasterApplication,
{
    let wrapped = TokioStream::new(stream);
    drive_master_session(&wrapped, link_cfg, master, soe, app, cmd_rx, ev_tx).await
}

/// Connect once to a TCP outstation at `addr` and spawn a Tokio master task.
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

    tokio::spawn(async move {
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
    });

    Ok((handle, ev_rx))
}

/// Spawn a TCP master that automatically reconnects with exponential backoff
/// whenever the connection drops or fails to establish.
pub fn spawn_tcp_master_reconnecting<H, A>(
    addr: SocketAddr,
    link_cfg: LinkConfig,
    params: MasterParams,
    retry: RetryStrategy,
    mut soe: H,
    mut app: A,
    state_tx: Option<mpsc::UnboundedSender<ChannelState>>,
) -> (MasterHandle, UnboundedReceiver<MasterEvent>)
where
    H: SoeHandler + Send + 'static,
    A: MasterApplication + Send + 'static,
{
    let (handle, cmd_rx) = master_channel();
    let (ev_tx, ev_rx) = unbounded();

    tokio::spawn(async move {
        let origin = Instant::now();
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut master = Master::new(params, addresses);
        let mut delay = retry.min_retry_delay;

        let notify = |s: ChannelState| {
            if let Some(ref tx) = state_tx {
                let _ = tx.send(s);
            }
        };

        loop {
            notify(ChannelState::Opening);
            let connect_res = tokio::select! {
                res = TcpStream::connect(addr) => Some(res),
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(MasterCommand::Shutdown) | None => {
                            notify(ChannelState::Shutdown);
                            return;
                        }
                        Some(other) => {
                            apply_master_command(&mut master, other, MonotonicTime::from_elapsed(origin), &mut app, &ev_tx);
                            None
                        }
                    }
                }
            };

            match connect_res {
                Some(Ok(stream)) => {
                    let _ = stream.set_nodelay(true);
                    delay = retry.min_retry_delay;
                    notify(ChannelState::Open);
                    let shutdown = run_master_session(
                        stream,
                        link_cfg.clone(),
                        &mut master,
                        &mut soe,
                        &mut app,
                        &cmd_rx,
                        &ev_tx,
                    )
                    .await;
                    if shutdown {
                        notify(ChannelState::Shutdown);
                        return;
                    }
                    notify(ChannelState::Closed);
                }
                Some(Err(_)) => {
                    notify(ChannelState::Closed);
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        cmd = cmd_rx.recv() => {
                            if matches!(cmd, Some(MasterCommand::Shutdown) | None) {
                                notify(ChannelState::Shutdown);
                                return;
                            }
                        }
                    }
                    delay = retry.next_delay(delay);
                }
                None => {}
            }
        }
    });

    (handle, ev_rx)
}

/// Spawn a TCP Master server accepting an incoming dial-out Outstation on `listener`.
pub fn spawn_tcp_master_server<H, A>(
    listener: TcpListener,
    link_cfg: LinkConfig,
    params: MasterParams,
    mut soe: H,
    mut app: A,
) -> (MasterHandle, UnboundedReceiver<MasterEvent>)
where
    H: SoeHandler + Send + 'static,
    A: MasterApplication + Send + 'static,
{
    let (handle, cmd_rx) = master_channel();
    let (ev_tx, ev_rx) = unbounded();

    tokio::spawn(async move {
        let origin = Instant::now();
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut master = Master::new(params, addresses);
        loop {
            let accept_res = tokio::select! {
                res = listener.accept() => Some(res),
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(MasterCommand::Shutdown) | None => break,
                        Some(other) => {
                            apply_master_command(&mut master, other, MonotonicTime::from_elapsed(origin), &mut app, &ev_tx);
                            None
                        }
                    }
                }
            };
            if let Some(Ok((stream, _peer))) = accept_res {
                let _ = stream.set_nodelay(true);
                let shutdown = run_master_session(
                    stream,
                    link_cfg.clone(),
                    &mut master,
                    &mut soe,
                    &mut app,
                    &cmd_rx,
                    &ev_tx,
                )
                .await;
                if shutdown {
                    break;
                }
            }
        }
    });

    (handle, ev_rx)
}
