//! Blocking `std::net` / OS-thread driver for [`opendnp3`] (zero async runtime).

use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use opendnp3::app::{Analog, Binary, ClassField, Counter};
use opendnp3::clock::MonotonicTime;
use opendnp3::enums::EventMode;
use opendnp3::link::{FrameParser, LinkConfig, LinkLayer};
use opendnp3::master::{Master, MasterApplication, MasterEvent, SoeHandler};
use opendnp3::outstation::{CommandHandler, Outstation, OutstationApplication};
use opendnp3::transport::{TransportEvent, TransportLayer};

/// Commands sent into [`run_blocking_master`].
#[derive(Debug)]
pub enum BlockingMasterCommand {
    ScanClasses(ClassField),
    Shutdown,
}

/// Commands sent into [`run_blocking_outstation`].
#[derive(Debug)]
pub enum BlockingOutstationCommand {
    UpdateBinary(u16, Binary, EventMode),
    UpdateAnalog(u16, Analog, EventMode),
    UpdateCounter(u16, Counter, EventMode),
    Shutdown,
}

/// Drive a [`Master`] over a blocking [`TcpStream`] on the current OS thread.
pub fn run_blocking_master<H: SoeHandler, A: MasterApplication>(
    mut stream: TcpStream,
    link_cfg: LinkConfig,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    cmd_rx: &Receiver<BlockingMasterCommand>,
    ev_tx: &Sender<MasterEvent>,
) -> io::Result<()> {
    let _ = stream.set_nodelay(true);
    stream.set_read_timeout(Some(Duration::from_millis(20)))?;

    let origin = Instant::now();
    let mut parser = FrameParser::new();
    let mut link = LinkLayer::new(link_cfg);
    let mut transport = TransportLayer::new(master.params.max_rx_frag_size);

    let now = MonotonicTime::from_elapsed(origin);
    link.on_lower_layer_up(now);
    pump_master(&mut link, &mut transport, master, soe, app, ev_tx, now);

    let mut buf = [0u8; 4096];
    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                BlockingMasterCommand::ScanClasses(c) => {
                    master.scan_classes(c, MonotonicTime::from_elapsed(origin), app);
                }
                BlockingMasterCommand::Shutdown => {
                    link.on_lower_layer_down();
                    pump_master(
                        &mut link,
                        &mut transport,
                        master,
                        soe,
                        app,
                        ev_tx,
                        MonotonicTime::from_elapsed(origin),
                    );
                    return Ok(());
                }
            }
        }

        flush_master_tx(
            &mut stream,
            &mut link,
            &mut transport,
            master,
            app,
            ev_tx,
            origin,
        )?;

        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let now = MonotonicTime::from_elapsed(origin);
                for rf in parser.push_bytes(&buf[..n]) {
                    link.on_frame(&rf.fields, &rf.payload, now);
                }
                pump_master(&mut link, &mut transport, master, soe, app, ev_tx, now);
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                let now = MonotonicTime::from_elapsed(origin);
                link.on_timeout(now);
                master.on_timeout(now, app);
                pump_master(&mut link, &mut transport, master, soe, app, ev_tx, now);
            }
            Err(e) => return Err(e),
        }
    }

    link.on_lower_layer_down();
    pump_master(
        &mut link,
        &mut transport,
        master,
        soe,
        app,
        ev_tx,
        MonotonicTime::from_elapsed(origin),
    );
    Ok(())
}

/// Drive an [`Outstation`] over a blocking [`TcpStream`] on the current OS thread.
pub fn run_blocking_outstation<C: CommandHandler, A: OutstationApplication>(
    mut stream: TcpStream,
    link_cfg: LinkConfig,
    outstation: &mut Outstation,
    cmd_handler: &mut C,
    app: &mut A,
    cmd_rx: &Receiver<BlockingOutstationCommand>,
) -> io::Result<()> {
    let _ = stream.set_nodelay(true);
    stream.set_read_timeout(Some(Duration::from_millis(20)))?;

    let origin = Instant::now();
    let mut parser = FrameParser::new();
    let mut link = LinkLayer::new(link_cfg);
    let mut transport = TransportLayer::new(outstation.config.params.max_rx_frag_size);

    let now = MonotonicTime::from_elapsed(origin);
    link.on_lower_layer_up(now);
    pump_outstation(&mut link, &mut transport, outstation, cmd_handler, app, now);

    let mut buf = [0u8; 4096];
    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            let now = MonotonicTime::from_elapsed(origin);
            match cmd {
                BlockingOutstationCommand::UpdateBinary(i, v, m) => {
                    outstation.database.update_binary(i, v, m);
                    outstation.check_for_task_start(now, app);
                }
                BlockingOutstationCommand::UpdateAnalog(i, v, m) => {
                    outstation.database.update_analog(i, v, m);
                    outstation.check_for_task_start(now, app);
                }
                BlockingOutstationCommand::UpdateCounter(i, v, m) => {
                    outstation.database.update_counter(i, v, m);
                    outstation.check_for_task_start(now, app);
                }
                BlockingOutstationCommand::Shutdown => {
                    link.on_lower_layer_down();
                    pump_outstation(&mut link, &mut transport, outstation, cmd_handler, app, now);
                    return Ok(());
                }
            }
        }

        flush_outstation_tx(
            &mut stream,
            &mut link,
            &mut transport,
            outstation,
            app,
            origin,
        )?;

        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let now = MonotonicTime::from_elapsed(origin);
                for rf in parser.push_bytes(&buf[..n]) {
                    link.on_frame(&rf.fields, &rf.payload, now);
                }
                pump_outstation(&mut link, &mut transport, outstation, cmd_handler, app, now);
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                let now = MonotonicTime::from_elapsed(origin);
                link.on_timeout(now);
                outstation.on_timeout(now, app);
                pump_outstation(&mut link, &mut transport, outstation, cmd_handler, app, now);
            }
            Err(e) => return Err(e),
        }
    }

    link.on_lower_layer_down();
    pump_outstation(
        &mut link,
        &mut transport,
        outstation,
        cmd_handler,
        app,
        MonotonicTime::from_elapsed(origin),
    );
    Ok(())
}

pub fn blocking_master_channel() -> (
    Sender<BlockingMasterCommand>,
    Receiver<BlockingMasterCommand>,
) {
    mpsc::channel()
}

pub fn blocking_outstation_channel() -> (
    Sender<BlockingOutstationCommand>,
    Receiver<BlockingOutstationCommand>,
) {
    mpsc::channel()
}

fn pump_master<H: SoeHandler, A: MasterApplication>(
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    master: &mut Master,
    soe: &mut H,
    app: &mut A,
    ev_tx: &Sender<MasterEvent>,
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

fn flush_master_tx<A: MasterApplication>(
    stream: &mut TcpStream,
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    master: &mut Master,
    app: &mut A,
    ev_tx: &Sender<MasterEvent>,
    origin: Instant,
) -> io::Result<()> {
    loop {
        let now = MonotonicTime::from_elapsed(origin);
        if !transport.is_sending()
            && let Some(apdu) = master.pop_transmit()
        {
            transport.begin_transmit(apdu, link, now);
        }
        if let Some(frame) = link.pop_transmit() {
            stream.write_all(&frame)?;
            stream.flush()?;
            link.on_tx_ready();
            for ev in link.drain_events() {
                transport.handle_link_event(&ev);
            }
            for ev in transport.drain_events() {
                if let TransportEvent::TxReady = ev {
                    master.on_tx_ready(MonotonicTime::from_elapsed(origin), app);
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

fn pump_outstation<C: CommandHandler, A: OutstationApplication>(
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
            TransportEvent::Message(msg) => outstation.on_receive(&msg, now, cmd_handler, app),
        }
    }
}

fn flush_outstation_tx<A: OutstationApplication>(
    stream: &mut TcpStream,
    link: &mut LinkLayer,
    transport: &mut TransportLayer,
    outstation: &mut Outstation,
    app: &mut A,
    origin: Instant,
) -> io::Result<()> {
    loop {
        let now = MonotonicTime::from_elapsed(origin);
        if !transport.is_sending()
            && let Some(apdu) = outstation.pop_transmit()
        {
            transport.begin_transmit(apdu, link, now);
        }
        if let Some(frame) = link.pop_transmit() {
            stream.write_all(&frame)?;
            stream.flush()?;
            link.on_tx_ready();
            for ev in link.drain_events() {
                transport.handle_link_event(&ev);
            }
            for ev in transport.drain_events() {
                if let TransportEvent::TxReady = ev {
                    outstation.on_tx_ready(MonotonicTime::from_elapsed(origin), app);
                }
            }
        } else {
            break;
        }
    }
    Ok(())
}
