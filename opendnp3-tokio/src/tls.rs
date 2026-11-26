//! TLS (tokio-rustls) transport helpers for DNP3 over TLS (IEEE 1815 / IEC 62351-3).

use std::io;
use std::net::SocketAddr;
use std::time::Instant;

use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{
    MasterHandle, OutstationCommand, OutstationHandle, UnboundedReceiver, apply_outstation_command,
    master_channel, outstation_channel, unbounded,
};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{Master, MasterApplication, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{CommandHandler, Outstation, OutstationApplication, OutstationConfig};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::master::run_master_session;
use crate::outstation::run_outstation_session;

/// Connect to a TLS outstation at `addr` with `server_name` and spawn a Master task.
pub async fn spawn_tls_master<H, A>(
    addr: SocketAddr,
    server_name: ServerName<'static>,
    connector: TlsConnector,
    link_cfg: LinkConfig,
    params: MasterParams,
    mut soe: H,
    mut app: A,
) -> io::Result<(MasterHandle, UnboundedReceiver<MasterEvent>)>
where
    H: SoeHandler + Send + 'static,
    A: MasterApplication + Send + 'static,
{
    let tcp = TcpStream::connect(addr).await?;
    let _ = tcp.set_nodelay(true);
    let tls_stream = connector.connect(server_name, tcp).await?;

    let (handle, cmd_rx) = master_channel();
    let (ev_tx, ev_rx) = unbounded();

    tokio::spawn(async move {
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut master = Master::new(params, addresses);
        let _ = run_master_session(
            tls_stream,
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

/// Spawn a TLS Outstation accepting connections on `listener` via `acceptor`.
pub fn spawn_tls_outstation<C, A>(
    listener: TcpListener,
    acceptor: TlsAcceptor,
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
            if let Some(Ok((tcp, _peer))) = accept_res {
                let _ = tcp.set_nodelay(true);
                if let Ok(tls_stream) = acceptor.accept(tcp).await {
                    let shutdown = run_outstation_session(
                        tls_stream,
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
        }
    });
    handle
}
