//! Runtime-agnostic, `no_std + alloc` async Master session drivers.

use alloc::boxed::Box;
use core::pin::pin;
use core::time::Duration;

use futures_util::future::{Either, pending, select};

use super::channel::{UnboundedReceiver, UnboundedSender, unbounded};
use super::stream::{AsyncStream, LocalAsyncStream, StreamError};
use crate::app::{ClassField, Range};
use crate::clock::MonotonicTime;
use crate::enums::{GroupVariation, PointClass, RestartType};
use crate::link::{FrameParser, LinkConfig, LinkLayer};
use crate::master::{CommandSet, Master, MasterApplication, MasterEvent, SoeHandler};
use crate::transport::{TransportEvent, TransportLayer};

/// Commands sent from [`MasterHandle`] into a running master session.
#[derive(Debug)]
pub enum MasterCommand {
    ScanClasses(ClassField),
    ScanRange(GroupVariation, Range),
    ScanAllObjects(GroupVariation),
    AddClassScan(ClassField, Duration),
    AddRangeScan(GroupVariation, Range, Duration),
    AddAllObjectsScan(GroupVariation, Duration),
    AssignClassAll(PointClass, GroupVariation),
    AssignClassRange(PointClass, GroupVariation, Range),
    SelectAndOperate(CommandSet),
    DirectOperate(CommandSet, bool),
    Restart(RestartType),
    EnableUnsolicited(ClassField),
    DisableUnsolicited(ClassField),
    Shutdown,
}

/// Cloneable handle for issuing scans and commands to a running [`Master`].
#[derive(Debug, Clone)]
pub struct MasterHandle {
    tx: UnboundedSender<MasterCommand>,
}

/// Create a paired [`MasterHandle`] and command receiver (`no_std + alloc`).
pub fn master_channel() -> (MasterHandle, UnboundedReceiver<MasterCommand>) {
    let (tx, rx) = unbounded();
    (MasterHandle { tx }, rx)
}

impl MasterHandle {
    pub fn from_sender(tx: UnboundedSender<MasterCommand>) -> Self {
        Self { tx }
    }

    pub fn scan_classes(&self, classes: ClassField) {
        let _ = self.tx.send(MasterCommand::ScanClasses(classes));
    }
    pub fn scan_range(&self, gv: GroupVariation, range: Range) {
        let _ = self.tx.send(MasterCommand::ScanRange(gv, range));
    }
    pub fn scan_all_objects(&self, gv: GroupVariation) {
        let _ = self.tx.send(MasterCommand::ScanAllObjects(gv));
    }
    pub fn add_class_scan(&self, classes: ClassField, period: Duration) {
        let _ = self.tx.send(MasterCommand::AddClassScan(classes, period));
    }
    pub fn add_range_scan(&self, gv: GroupVariation, range: Range, period: Duration) {
        let _ = self.tx.send(MasterCommand::AddRangeScan(gv, range, period));
    }
    pub fn add_all_objects_scan(&self, gv: GroupVariation, period: Duration) {
        let _ = self.tx.send(MasterCommand::AddAllObjectsScan(gv, period));
    }
    pub fn assign_class_all(&self, class: PointClass, gv: GroupVariation) {
        let _ = self.tx.send(MasterCommand::AssignClassAll(class, gv));
    }
    pub fn assign_class_range(&self, class: PointClass, gv: GroupVariation, range: Range) {
        let _ = self
            .tx
            .send(MasterCommand::AssignClassRange(class, gv, range));
    }
    pub fn select_and_operate(&self, commands: CommandSet) {
        let _ = self.tx.send(MasterCommand::SelectAndOperate(commands));
    }
    pub fn direct_operate(&self, commands: CommandSet, no_ack: bool) {
        let _ = self.tx.send(MasterCommand::DirectOperate(commands, no_ack));
    }
    pub fn restart(&self, restart_type: RestartType) {
        let _ = self.tx.send(MasterCommand::Restart(restart_type));
    }
    pub fn enable_unsolicited(&self, classes: ClassField) {
        let _ = self.tx.send(MasterCommand::EnableUnsolicited(classes));
    }
    pub fn disable_unsolicited(&self, classes: ClassField) {
        let _ = self.tx.send(MasterCommand::DisableUnsolicited(classes));
    }
    pub fn shutdown(&self) {
        let _ = self.tx.send(MasterCommand::Shutdown);
    }
}

/// Apply a single [`MasterCommand`] directly to a [`Master`] instance.
pub fn apply_master_command<A: MasterApplication>(
    master: &mut Master,
    cmd: MasterCommand,
    now: MonotonicTime,
    app: &mut A,
    ev_tx: &UnboundedSender<MasterEvent>,
) {
    match cmd {
        MasterCommand::ScanClasses(c) => master.scan_classes(c, now, app),
        MasterCommand::ScanRange(gv, r) => master.scan_range(gv, r, now, app),
        MasterCommand::ScanAllObjects(gv) => master.scan_all_objects(gv, now, app),
        MasterCommand::AddClassScan(c, p) => master.add_class_scan(c, p, now),
        MasterCommand::AddRangeScan(gv, r, p) => master.add_range_scan(gv, r, p, now),
        MasterCommand::AddAllObjectsScan(gv, p) => master.add_all_objects_scan(gv, p, now),
        MasterCommand::AssignClassAll(c, gv) => master.assign_class_all(c, gv, now, app),
        MasterCommand::AssignClassRange(c, gv, r) => master.assign_class_range(c, gv, r, now, app),
        MasterCommand::SelectAndOperate(cs) => master.select_and_operate(cs, now, app),
        MasterCommand::DirectOperate(cs, nr) => master.direct_operate(cs, nr, now, app),
        MasterCommand::Restart(rt) => master.restart(rt, now, app),
        MasterCommand::EnableUnsolicited(c) => master.enable_unsolicited(c, now, app),
        MasterCommand::DisableUnsolicited(c) => master.disable_unsolicited(c, now, app),
        MasterCommand::Shutdown => {}
    }
    for ev in master.drain_events() {
        let _ = ev_tx.send(ev);
    }
}

macro_rules! impl_master_session_loop {
    ($stream:expr, $link_cfg:expr, $master:expr, $soe:expr, $app:expr, $cmd_rx:expr, $ev_tx:expr) => {{
        let mut parser = FrameParser::new();
        let mut link = LinkLayer::new($link_cfg);
        let mut transport = TransportLayer::new($master.params.max_rx_frag_size);

        let now = $stream.now();
        link.on_lower_layer_up(now);
        pump_events(&mut link, &mut transport, $master, $soe, $app, $ev_tx, now);

        let mut shutdown_requested = false;
        let mut read_fut = Box::pin($stream.read(4096));

        loop {
            if flush_tx_inner!($stream, &mut link, &mut transport, $master, $app, $ev_tx).is_err() {
                break;
            }
            pump_events(
                &mut link,
                &mut transport,
                $master,
                $soe,
                $app,
                $ev_tx,
                $stream.now(),
            );
            if flush_tx_inner!($stream, &mut link, &mut transport, $master, $app, $ev_tx).is_err() {
                break;
            }

            let next_wakeup = earliest(link.next_wakeup(), $master.next_wakeup());
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
                        pump_events(&mut link, &mut transport, $master, $soe, $app, $ev_tx, now);
                        read_fut.set($stream.read(4096));
                    }
                    _ => break,
                },
                Either::Left((Either::Right((cmd_opt, _)), _)) => {
                    let now = $stream.now();
                    match cmd_opt {
                        Some(MasterCommand::Shutdown) | None => {
                            shutdown_requested = true;
                            break;
                        }
                        Some(other) => {
                            apply_master_command($master, other, now, $app, $ev_tx);
                        }
                    }
                }
                Either::Right(((), _)) => {
                    let now = $stream.now();
                    link.on_timeout(now);
                    $master.on_timeout(now, $app);
                    pump_events(&mut link, &mut transport, $master, $soe, $app, $ev_tx, now);
                }
            }
        }

        link.on_lower_layer_down();
        pump_events(
            &mut link,
            &mut transport,
            $master,
            $soe,
            $app,
            $ev_tx,
            $stream.now(),
        );
        shutdown_requested
    }};
}

macro_rules! flush_tx_inner {
    ($stream:expr, $link:expr, $transport:expr, $master:expr, $app:expr, $ev_tx:expr) => {{
        let mut res: Result<(), StreamError> = Ok(());
        loop {
            let now = $stream.now();
            if !$transport.is_sending()
                && let Some(apdu) = $master.pop_transmit()
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
                        $master.on_tx_ready($stream.now(), $app);
                    }
                }
                for ev in $master.drain_events() {
                    let _ = $ev_tx.send(ev);
                }
            } else {
                break;
            }
        }
        res
    }};
}

/// Drive a single connected [`AsyncStream`] (`Send` futures: Tokio, smol, BoltFFI).
pub async fn run_master_session<S, H, A>(
    stream: &S,
    link_cfg: LinkConfig,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<MasterCommand>,
    ev_tx: &UnboundedSender<MasterEvent>,
) -> bool
where
    S: AsyncStream,
    H: SoeHandler,
    A: MasterApplication,
{
    impl_master_session_loop!(stream, link_cfg, master, soe, app, cmd_rx, ev_tx)
}

/// Drive a single connected [`LocalAsyncStream`] (`!Send` ok: Embassy, compio).
pub async fn run_master_session_local<S, H, A>(
    stream: &S,
    link_cfg: LinkConfig,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    cmd_rx: &UnboundedReceiver<MasterCommand>,
    ev_tx: &UnboundedSender<MasterEvent>,
) -> bool
where
    S: LocalAsyncStream,
    H: SoeHandler,
    A: MasterApplication,
{
    impl_master_session_loop!(stream, link_cfg, master, soe, app, cmd_rx, ev_tx)
}

fn pump_events<H: SoeHandler, A: MasterApplication>(
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    ev_tx: &UnboundedSender<MasterEvent>,
    now: MonotonicTime,
) {
    for ev in link.drain_events() {
        transport.handle_link_event(&ev);
    }
    for ev in transport.drain_events() {
        match ev {
            TransportEvent::LowerLayerUp => master.on_lower_layer_up(now, app),
            TransportEvent::LowerLayerDown => master.on_lower_layer_down(app),
            TransportEvent::TxReady => master.on_tx_ready(now, app),
            TransportEvent::Message(msg) => master.on_receive(&msg, now, soe, app),
        }
    }
    for ev in master.drain_events() {
        let _ = ev_tx.send(ev);
    }
}

fn earliest(a: Option<MonotonicTime>, b: Option<MonotonicTime>) -> Option<MonotonicTime> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}
