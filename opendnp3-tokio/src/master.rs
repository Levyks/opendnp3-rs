//! Async Tokio Master driver over any `AsyncRead + AsyncWrite` stream or TCP connection,
//! including automatic exponential-backoff reconnection.

use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use opendnp3::app::{ClassField, Range};
use opendnp3::enums::{ChannelState, GroupVariation, PointClass, RestartType};
use opendnp3::link::{Addresses, FrameParser, LinkConfig, LinkLayer};
use opendnp3::master::{
    CommandSet, Master, MasterApplication, MasterEvent, MasterParams, SoeHandler,
};
use opendnp3::transport::{TransportEvent, TransportLayer};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

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

/// Commands sent from [`MasterHandle`] into the running master task.
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

/// Cloneable handle for issuing scans and commands on a running master.
#[derive(Debug, Clone)]
pub struct MasterHandle {
    tx: mpsc::UnboundedSender<MasterCommand>,
}

impl MasterHandle {
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

/// Connect once to a TCP outstation at `addr` and spawn a Tokio master task.
pub async fn spawn_tcp_master<H, A>(
    addr: SocketAddr,
    link_cfg: LinkConfig,
    params: MasterParams,
    mut soe: H,
    mut app: A,
) -> io::Result<(MasterHandle, mpsc::UnboundedReceiver<MasterEvent>)>
where
    H: SoeHandler + Send + 'static,
    A: MasterApplication + Send + 'static,
{
    let stream = TcpStream::connect(addr).await?;
    let _ = stream.set_nodelay(true);
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
    let (ev_tx, ev_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let mut master = Master::new(params, addresses);
        let _ = run_master_session(
            stream,
            link_cfg,
            &mut master,
            &mut soe,
            &mut app,
            &mut cmd_rx,
            &ev_tx,
        )
        .await;
    });

    Ok((MasterHandle { tx: cmd_tx }, ev_rx))
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
) -> (MasterHandle, mpsc::UnboundedReceiver<MasterEvent>)
where
    H: SoeHandler + Send + 'static,
    A: MasterApplication + Send + 'static,
{
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
    let (ev_tx, ev_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
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
                            apply_master_command(&mut master, other, Instant::now(), &mut app, &ev_tx);
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
                        &mut cmd_rx,
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

    (MasterHandle { tx: cmd_tx }, ev_rx)
}

/// Drive a single connected byte stream for a [`Master`].
/// Returns `true` if `Shutdown` was requested, `false` if the stream closed.
pub async fn run_master_session<S, H, A>(
    mut stream: S,
    link_cfg: LinkConfig,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    cmd_rx: &mut mpsc::UnboundedReceiver<MasterCommand>,
    ev_tx: &mpsc::UnboundedSender<MasterEvent>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Unpin,
    H: SoeHandler,
    A: MasterApplication,
{
    let mut parser = FrameParser::new();
    let mut link = LinkLayer::new(link_cfg);
    let mut transport = TransportLayer::new(master.params.max_rx_frag_size);

    let now = Instant::now();
    link.on_lower_layer_up(now);
    pump_events(&mut link, &mut transport, master, soe, app, ev_tx, now);

    let mut read_buf = [0u8; 4096];
    let mut shutdown_requested = false;

    loop {
        if flush_tx(&mut stream, &mut link, &mut transport, master, app, ev_tx)
            .await
            .is_err()
        {
            break;
        }
        pump_events(
            &mut link,
            &mut transport,
            master,
            soe,
            app,
            ev_tx,
            Instant::now(),
        );
        if flush_tx(&mut stream, &mut link, &mut transport, master, app, ev_tx)
            .await
            .is_err()
        {
            break;
        }

        let next_wakeup = earliest(link.next_wakeup(), master.next_wakeup());
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
                        pump_events(&mut link, &mut transport, master, soe, app, ev_tx, now);
                    }
                }
            }
            cmd = cmd_rx.recv() => {
                let now = Instant::now();
                match cmd {
                    Some(MasterCommand::Shutdown) | None => {
                        shutdown_requested = true;
                        break;
                    }
                    Some(other) => {
                        apply_master_command(master, other, now, app, ev_tx);
                    }
                }
            }
            _ = sleep_fut => {
                let now = Instant::now();
                link.on_timeout(now);
                master.on_timeout(now, app);
                pump_events(&mut link, &mut transport, master, soe, app, ev_tx, now);
            }
        }
    }

    link.on_lower_layer_down();
    pump_events(
        &mut link,
        &mut transport,
        master,
        soe,
        app,
        ev_tx,
        Instant::now(),
    );
    shutdown_requested
}

fn apply_master_command<A: MasterApplication>(
    master: &mut Master,
    cmd: MasterCommand,
    now: Instant,
    app: &mut A,
    ev_tx: &mpsc::UnboundedSender<MasterEvent>,
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

fn pump_events<H: SoeHandler, A: MasterApplication>(
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    ev_tx: &mpsc::UnboundedSender<MasterEvent>,
    now: Instant,
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

async fn flush_tx<S, A>(
    stream: &mut S,
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    master: &mut Master,
    app: &mut A,
    ev_tx: &mpsc::UnboundedSender<MasterEvent>,
) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
    A: MasterApplication,
{
    loop {
        let now = Instant::now();
        if !transport.is_sending()
            && let Some(apdu) = master.pop_transmit()
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
                    master.on_tx_ready(Instant::now(), app);
                }
            }
            for ev in master.drain_events() {
                let _ = ev_tx.send(ev);
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
