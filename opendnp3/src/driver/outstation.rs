//! Runtime-agnostic, `no_std + alloc` async Outstation session drivers.

use alloc::boxed::Box;
use core::pin::pin;

use futures_util::future::{Either, pending, select};

use super::channel::{UnboundedReceiver, UnboundedSender, unbounded};
use super::stream::{AsyncStream, LocalAsyncStream, StreamError};
use crate::app::{
    Analog, AnalogOutputStatus, Binary, BinaryOutputStatus, Counter, DoubleBitBinary,
    FrozenCounter, OctetString,
};
use crate::clock::MonotonicTime;
use crate::enums::EventMode;
use crate::link::{FrameParser, LinkConfig, LinkLayer};
use crate::outstation::{CommandHandler, Outstation, OutstationApplication};
use crate::transport::{TransportEvent, TransportLayer};

/// Commands sent from [`OutstationHandle`] into a running outstation session.
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

/// Cloneable handle for pushing measurement updates into a running [`Outstation`].
#[derive(Debug, Clone)]
pub struct OutstationHandle {
    tx: UnboundedSender<OutstationCommand>,
}

/// Create a paired [`OutstationHandle`] and command receiver (`no_std + alloc`).
pub fn outstation_channel() -> (OutstationHandle, UnboundedReceiver<OutstationCommand>) {
    let (tx, rx) = unbounded();
    (OutstationHandle { tx }, rx)
}

impl OutstationHandle {
    pub fn from_sender(tx: UnboundedSender<OutstationCommand>) -> Self {
        Self { tx }
    }

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

/// Apply a single [`OutstationCommand`] directly to an [`Outstation`] instance.
pub fn apply_outstation_command(
    outstation: &mut Outstation,
    cmd: OutstationCommand,
    now: MonotonicTime,
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

macro_rules! impl_outstation_session_loop {
    ($stream:expr, $link_cfg:expr, $outstation:expr, $cmd_handler:expr, $app:expr, $cmd_rx:expr) => {{
        let mut parser = FrameParser::new();
        let mut link = LinkLayer::new($link_cfg);
        let mut transport = TransportLayer::new($outstation.config.params.max_rx_frag_size);

        let now = $stream.now();
        link.on_lower_layer_up(now);
        pump_events(
            &mut link,
            &mut transport,
            $outstation,
            $cmd_handler,
            $app,
            now,
        );

        let mut shutdown_requested = false;
        while let Some(cmd) = $cmd_rx.try_recv() {
            if matches!(cmd, OutstationCommand::Shutdown) {
                shutdown_requested = true;
                break;
            }
            apply_outstation_command($outstation, cmd, $stream.now(), $app);
        }

        let mut read_fut = Box::pin($stream.read(4096));

        while !shutdown_requested {
            if flush_tx_inner!($stream, &mut link, &mut transport, $outstation, $app).is_err() {
                break;
            }
            pump_events(
                &mut link,
                &mut transport,
                $outstation,
                $cmd_handler,
                $app,
                $stream.now(),
            );
            if flush_tx_inner!($stream, &mut link, &mut transport, $outstation, $app).is_err() {
                break;
            }

            let next_wakeup = earliest(link.next_wakeup(), $outstation.next_wakeup());
            let now = $stream.now();
            let sleep_fut = pin!(async {
                match next_wakeup {
                    Some(deadline) => {
                        let dur = deadline.saturating_duration_since(now);
                        if !dur.is_zero() {
                            $stream.sleep(dur).await;
                        }
                    }
                    None => pending::<()>().await,
                }
            });

            let cmd_fut = $cmd_rx.recv();
            let io_or_cmd = select(&mut read_fut, cmd_fut);
            match select(io_or_cmd, sleep_fut).await {
                Either::Left((Either::Left((read_res, _)), _)) => match read_res {
                    Ok(bytes) if !bytes.is_empty() => {
                        let now = $stream.now();
                        for rf in parser.push_bytes(&bytes) {
                            link.on_frame(&rf.fields, &rf.payload, now);
                        }
                        pump_events(
                            &mut link,
                            &mut transport,
                            $outstation,
                            $cmd_handler,
                            $app,
                            now,
                        );
                        read_fut.set($stream.read(4096));
                    }
                    _ => break,
                },
                Either::Left((Either::Right((cmd_opt, _)), _)) => match cmd_opt {
                    Some(OutstationCommand::Shutdown) | None => {
                        shutdown_requested = true;
                        break;
                    }
                    Some(other) => {
                        apply_outstation_command($outstation, other, $stream.now(), $app);
                    }
                },
                Either::Right(((), _)) => {
                    let now = $stream.now();
                    link.on_timeout(now);
                    $outstation.on_timeout(now, $app);
                    pump_events(
                        &mut link,
                        &mut transport,
                        $outstation,
                        $cmd_handler,
                        $app,
                        now,
                    );
                }
            }
        }

        link.on_lower_layer_down();
        pump_events(
            &mut link,
            &mut transport,
            $outstation,
            $cmd_handler,
            $app,
            $stream.now(),
        );
        shutdown_requested
    }};
}

macro_rules! flush_tx_inner {
    ($stream:expr, $link:expr, $transport:expr, $outstation:expr, $app:expr) => {{
        let mut res: Result<(), StreamError> = Ok(());
        loop {
            let now = $stream.now();
            if !$transport.is_sending()
                && let Some(apdu) = $outstation.pop_transmit()
            {
                $transport.begin_transmit(apdu, $link, now);
            }
            if let Some(frame) = $link.pop_transmit() {
                if let Err(e) = $stream.write(&frame).await {
                    res = Err(e);
                    break;
                }
                $link.on_tx_ready();
                for ev in $link.drain_events() {
                    $transport.handle_link_event(&ev);
                }
                for ev in $transport.drain_events() {
                    if let TransportEvent::TxReady = ev {
                        $outstation.on_tx_ready($stream.now(), $app);
                    }
                }
            } else {
                break;
            }
        }
        res
    }};
}

/// Drive a single connected [`AsyncStream`] (`Send` futures: Tokio, smol, BoltFFI).
pub async fn run_outstation_session<S, C, A>(
    stream: &S,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<OutstationCommand>,
) -> bool
where
    S: AsyncStream,
    C: CommandHandler,
    A: OutstationApplication,
{
    impl_outstation_session_loop!(stream, link_cfg, outstation, cmd_handler, app, cmd_rx)
}

/// Drive a single connected [`LocalAsyncStream`] (`!Send` ok: Embassy, compio).
pub async fn run_outstation_session_local<S, C, A>(
    stream: &S,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<OutstationCommand>,
) -> bool
where
    S: LocalAsyncStream,
    C: CommandHandler,
    A: OutstationApplication,
{
    impl_outstation_session_loop!(stream, link_cfg, outstation, cmd_handler, app, cmd_rx)
}

fn pump_events<C: CommandHandler, A: OutstationApplication>(
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    now: MonotonicTime,
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

fn earliest(a: Option<MonotonicTime>, b: Option<MonotonicTime>) -> Option<MonotonicTime> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}
