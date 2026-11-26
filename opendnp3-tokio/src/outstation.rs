//! Tokio Outstation helpers built on top of [`opendnp3::driver::run_outstation_session`].

use std::time::Instant;

use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{
    OutstationCommand, OutstationHandle, UnboundedReceiver, apply_outstation_command,
    outstation_channel, run_outstation_session as drive_outstation_session,
};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::outstation::{CommandHandler, Outstation, OutstationApplication, OutstationConfig};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;

use crate::stream::TokioStream;

/// Drive a single connected Tokio `AsyncRead + AsyncWrite` stream for an [`Outstation`].
/// Returns `true` if `Shutdown` was requested, `false` if the stream closed.
pub async fn run_outstation_session<S, C, A>(
    stream: S,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    rx: &UnboundedReceiver<OutstationCommand>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Send + 'static,
    C: CommandHandler,
    A: OutstationApplication,
{
    let wrapped = TokioStream::new(stream);
    drive_outstation_session(&wrapped, link_cfg, outstation, cmd_handler, app, rx).await
}

/// Spawn a TCP outstation accepting connections on `listener`.
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
    tokio::spawn(async move {
        let origin = Instant::now();
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut outstation = Outstation::new(outstation_cfg, addresses);
        loop {
            let accept_res = tokio::select! {
                res = listener.accept() => Some(res),
                cmd = rx.recv() => {
                    match cmd {
                        Some(OutstationCommand::Shutdown) | None => break,
                        Some(other) => {
                            apply_outstation_command(&mut outstation, other, MonotonicTime::from_elapsed(origin), &app);
                            None
                        }
                    }
                }
            };
            if let Some(Ok((stream, _peer))) = accept_res {
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
        }
    });
    handle
}
