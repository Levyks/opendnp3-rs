//! Async Tokio Outstation driver over any `AsyncRead + AsyncWrite` stream or TCP listener.

use std::io;
use std::time::Instant;

use opendnp3::app::{
    Analog, AnalogOutputStatus, Binary, BinaryOutputStatus, Counter, DoubleBitBinary,
    FrozenCounter, OctetString,
};
use opendnp3::enums::EventMode;
use opendnp3::link::{Addresses, FrameParser, LinkConfig, LinkLayer};
use opendnp3::outstation::{CommandHandler, Outstation, OutstationApplication, OutstationConfig};
use opendnp3::transport::{TransportEvent, TransportLayer};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// Commands sent from [`OutstationHandle`] into the running outstation task.
#[derive(Debug)]
pub enum OutstationCommand {
    UpdateBinary(u16, Binary, EventMode),
    UpdateDoubleBit(u16, DoubleBitBinary, EventMode),
    UpdateBinaryOutputStatus(u16, BinaryOutputStatus, EventMode),
    UpdateCounter(u16, Counter, EventMode),
    UpdateFrozenCounter(u16, FrozenCounter, EventMode),
    UpdateAnalog(u16, Analog, EventMode),
    UpdateAnalogOutputStatus(u16, AnalogOutputStatus, EventMode),
    UpdateOctetString(u16, OctetString, EventMode),
    Shutdown,
}

/// Cloneable handle for pushing measurement updates into a running outstation.
#[derive(Debug, Clone)]
pub struct OutstationHandle {
    tx: mpsc::UnboundedSender<OutstationCommand>,
}

impl OutstationHandle {
    pub fn update_binary(&self, index: u16, value: Binary, mode: EventMode) {
        let _ = self
            .tx
            .send(OutstationCommand::UpdateBinary(index, value, mode));
    }
    pub fn update_double_bit(&self, index: u16, value: DoubleBitBinary, mode: EventMode) {
        let _ = self
            .tx
            .send(OutstationCommand::UpdateDoubleBit(index, value, mode));
    }
    pub fn update_binary_output_status(
        &self,
        index: u16,
        value: BinaryOutputStatus,
        mode: EventMode,
    ) {
        let _ = self.tx.send(OutstationCommand::UpdateBinaryOutputStatus(
            index, value, mode,
        ));
    }
    pub fn update_counter(&self, index: u16, value: Counter, mode: EventMode) {
        let _ = self
            .tx
            .send(OutstationCommand::UpdateCounter(index, value, mode));
    }
    pub fn update_frozen_counter(&self, index: u16, value: FrozenCounter, mode: EventMode) {
        let _ = self
            .tx
            .send(OutstationCommand::UpdateFrozenCounter(index, value, mode));
    }
    pub fn update_analog(&self, index: u16, value: Analog, mode: EventMode) {
        let _ = self
            .tx
            .send(OutstationCommand::UpdateAnalog(index, value, mode));
    }
    pub fn update_analog_output_status(
        &self,
        index: u16,
        value: AnalogOutputStatus,
        mode: EventMode,
    ) {
        let _ = self.tx.send(OutstationCommand::UpdateAnalogOutputStatus(
            index, value, mode,
        ));
    }
    pub fn update_octet_string(&self, index: u16, value: OctetString, mode: EventMode) {
        let _ = self
            .tx
            .send(OutstationCommand::UpdateOctetString(index, value, mode));
    }
    pub fn shutdown(&self) {
        let _ = self.tx.send(OutstationCommand::Shutdown);
    }
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
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut outstation = Outstation::new(outstation_cfg, addresses);
        loop {
            let accept_res = tokio::select! {
                res = listener.accept() => Some(res),
                cmd = rx.recv() => {
                    match cmd {
                        Some(OutstationCommand::Shutdown) | None => break,
                        Some(other) => {
                            apply_command(&mut outstation, other, Instant::now(), &app);
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
                    &mut rx,
                )
                .await;
                if shutdown {
                    break;
                }
            }
        }
    });
    OutstationHandle { tx }
}

/// Drive a single connected byte stream for an [`Outstation`].
/// Returns `true` if `Shutdown` was requested, `false` if the stream closed.
pub async fn run_outstation_session<S, C, A>(
    mut stream: S,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    rx: &mut mpsc::UnboundedReceiver<OutstationCommand>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Unpin,
    C: CommandHandler,
    A: OutstationApplication,
{
    let mut parser = FrameParser::new();
    let mut link = LinkLayer::new(link_cfg);
    let mut transport = TransportLayer::new(outstation.config.params.max_rx_frag_size);

    let now = Instant::now();
    link.on_lower_layer_up(now);
    pump_events(&mut link, &mut transport, outstation, cmd_handler, app, now);

    let mut read_buf = [0u8; 4096];

    loop {
        if flush_tx(&mut stream, &mut link, &mut transport, outstation, app)
            .await
            .is_err()
        {
            break;
        }
        pump_events(
            &mut link,
            &mut transport,
            outstation,
            cmd_handler,
            app,
            Instant::now(),
        );
        if flush_tx(&mut stream, &mut link, &mut transport, outstation, app)
            .await
            .is_err()
        {
            break;
        }

        let next_wakeup = earliest(link.next_wakeup(), outstation.next_wakeup());
        let sleep_fut = async {
            match next_wakeup {
                Some(deadline) => {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
                }
                None => std::future::pending::<()>().await,
            }
        };

        tokio::select! {
            read_res = stream.read(&mut read_buf) => {
                match read_res {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let now = Instant::now();
                        for rf in parser.push_bytes(&read_buf[..n]) {
                            link.on_frame(&rf.fields, &rf.payload, now);
                        }
                        pump_events(&mut link, &mut transport, outstation, cmd_handler, app, now);
                    }
                }
            }
            cmd = rx.recv() => {
                match cmd {
                    Some(OutstationCommand::Shutdown) | None => {
                        link.on_lower_layer_down();
                        pump_events(&mut link, &mut transport, outstation, cmd_handler, app, Instant::now());
                        return true;
                    }
                    Some(other) => {
                        apply_command(outstation, other, Instant::now(), app);
                    }
                }
            }
            _ = sleep_fut => {
                let now = Instant::now();
                link.on_timeout(now);
                outstation.on_timeout(now, app);
                pump_events(&mut link, &mut transport, outstation, cmd_handler, app, now);
            }
        }
    }

    link.on_lower_layer_down();
    pump_events(
        &mut link,
        &mut transport,
        outstation,
        cmd_handler,
        app,
        Instant::now(),
    );
    false
}

fn apply_command(
    outstation: &mut Outstation,
    cmd: OutstationCommand,
    now: Instant,
    app: &dyn OutstationApplication,
) {
    match cmd {
        OutstationCommand::UpdateBinary(i, v, m) => {
            outstation.database.update_binary(i, v, m);
        }
        OutstationCommand::UpdateDoubleBit(i, v, m) => {
            outstation.database.update_double_bit(i, v, m);
        }
        OutstationCommand::UpdateBinaryOutputStatus(i, v, m) => {
            outstation.database.update_binary_output_status(i, v, m);
        }
        OutstationCommand::UpdateCounter(i, v, m) => {
            outstation.database.update_counter(i, v, m);
        }
        OutstationCommand::UpdateFrozenCounter(i, v, m) => {
            outstation.database.update_frozen_counter(i, v, m);
        }
        OutstationCommand::UpdateAnalog(i, v, m) => {
            outstation.database.update_analog(i, v, m);
        }
        OutstationCommand::UpdateAnalogOutputStatus(i, v, m) => {
            outstation.database.update_analog_output_status(i, v, m);
        }
        OutstationCommand::UpdateOctetString(i, v, m) => {
            outstation.database.update_octet_string(i, v, m);
        }
        OutstationCommand::Shutdown => return,
    }
    outstation.check_for_task_start(now, app);
}

fn pump_events<C: CommandHandler, A: OutstationApplication>(
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    now: Instant,
) {
    for ev in link.drain_events() {
        transport.handle_link_event(&ev);
    }
    for ev in transport.drain_events() {
        match ev {
            TransportEvent::LowerLayerUp => outstation.on_lower_layer_up(now, app),
            TransportEvent::LowerLayerDown => outstation.on_lower_layer_down(),
            TransportEvent::TxReady => outstation.on_tx_ready(now, app),
            TransportEvent::Message(msg) => {
                outstation.on_receive(&msg, now, cmd_handler, app);
            }
        }
    }
}

async fn flush_tx<S, A>(
    stream: &mut S,
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    outstation: &mut Outstation,
    app: &mut A,
) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
    A: OutstationApplication,
{
    loop {
        let now = Instant::now();
        if !transport.is_sending()
            && let Some(apdu) = outstation.pop_transmit()
        {
            transport.begin_transmit(apdu, link, now);
        }
        if let Some(frame) = link.pop_transmit() {
            stream.write_all(&frame).await?;
            stream.flush().await?;
            link.on_tx_ready();
            for ev in link.drain_events() {
                transport.handle_link_event(&ev);
            }
            for ev in transport.drain_events() {
                if let TransportEvent::TxReady = ev {
                    outstation.on_tx_ready(Instant::now(), app);
                }
            }
        } else {
            break;
        }
    }
    Ok(())
}

fn earliest(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}
