//! End-to-end Embassy integration test using `embedded-io-async` + `embassy-time`.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use embedded_io_async::{ErrorType, Read, Write};
use futures_util::future::join3;
use opendnp3::app::{Analog, Binary, HeaderInfo, Indexed};
use opendnp3::driver::{
    UnboundedReceiver, UnboundedSender, master_channel, outstation_channel, unbounded,
};
use opendnp3::enums::{EventMode, MasterTaskType, TaskCompletion};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{DefaultMasterApplication, Master, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{
    DatabaseConfig, DefaultCommandHandler, DefaultOutstationApplication, Outstation,
    OutstationConfig,
};
use opendnp3_embassy::{EmbassyStream, run_embassy_master, run_embassy_outstation};

struct ThreadWaker(Thread);
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => thread::park(),
        }
    }
}

struct ChanReader {
    rx: UnboundedReceiver<Vec<u8>>,
    pending: Vec<u8>,
}

struct ChanWriter {
    tx: UnboundedSender<Vec<u8>>,
}

impl ErrorType for ChanReader {
    type Error = embedded_io_async::ErrorKind;
}
impl ErrorType for ChanWriter {
    type Error = embedded_io_async::ErrorKind;
}

impl Read for ChanReader {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if self.pending.is_empty() {
            match self.rx.recv().await {
                Some(chunk) => self.pending = chunk,
                None => return Ok(0),
            }
        }
        let n = buf.len().min(self.pending.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

impl Write for ChanWriter {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.tx
            .send(buf.to_vec())
            .map_err(|_| embedded_io_async::ErrorKind::BrokenPipe)?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingSoe {
    binaries: Vec<Indexed<Binary>>,
    analogs: Vec<Indexed<Analog>>,
}

impl SoeHandler for RecordingSoe {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.extend_from_slice(values);
    }
    fn process_analog(&mut self, _info: HeaderInfo, values: &[Indexed<Analog>]) {
        self.analogs.extend_from_slice(values);
    }
}

#[test]
fn embassy_stream_master_and_outstation_end_to_end() {
    let (a_tx, b_rx) = unbounded();
    let (b_tx, a_rx) = unbounded();
    let m_stream = EmbassyStream::new(
        ChanReader {
            rx: a_rx,
            pending: Vec::new(),
        },
        ChanWriter { tx: a_tx },
    );
    let o_stream = EmbassyStream::new(
        ChanReader {
            rx: b_rx,
            pending: Vec::new(),
        },
        ChanWriter { tx: b_tx },
    );

    let mut m_link = LinkConfig::new(true);
    m_link.local_addr = 1;
    m_link.remote_addr = 10;
    let mut o_link = LinkConfig::new(false);
    o_link.local_addr = 10;
    o_link.remote_addr = 1;

    let mut master = Master::new(MasterParams::default(), Addresses::new(1, 10));
    let mut m_soe = RecordingSoe::default();
    let mut m_app = DefaultMasterApplication;
    let (m_handle, m_cmd_rx) = master_channel();
    let (ev_tx, ev_rx) = unbounded();

    let o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(3),
        ..Default::default()
    };
    let mut outstation = Outstation::new(o_cfg, Addresses::new(10, 1));
    outstation
        .database
        .update_binary(0, Binary::new(true), EventMode::Suppress);
    outstation
        .database
        .update_analog(2, Analog::new(321.0), EventMode::Suppress);
    let mut o_cmd = DefaultCommandHandler;
    let mut o_app = DefaultOutstationApplication;
    let (o_handle, o_cmd_rx) = outstation_channel();

    let m_task = run_embassy_master(
        &m_stream,
        m_link,
        &mut master,
        &mut m_soe,
        &mut m_app,
        &m_cmd_rx,
        &ev_tx,
    );
    let o_task = run_embassy_outstation(
        &o_stream,
        o_link,
        &mut outstation,
        &mut o_cmd,
        &mut o_app,
        &o_cmd_rx,
    );
    let observer = async move {
        while let Some(ev) = ev_rx.recv().await {
            if matches!(
                ev,
                MasterEvent::TaskComplete {
                    task_type: MasterTaskType::StartupIntegrityPoll,
                    result: TaskCompletion::Success,
                }
            ) {
                m_handle.shutdown();
                o_handle.shutdown();
                break;
            }
        }
    };

    let (m_shutdown, o_shutdown, ()) = block_on(join3(m_task, o_task, observer));
    assert!(m_shutdown);
    assert!(o_shutdown);
    assert!(m_soe.binaries.iter().any(|b| b.index == 0 && b.value.value));
    assert!(
        m_soe
            .analogs
            .iter()
            .any(|a| a.index == 2 && (a.value.value - 321.0).abs() < 1e-6)
    );
}
