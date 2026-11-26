//! End-to-end blocking `std::net` TCP integration test (zero async runtime).

use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use opendnp3::app::{Binary, Counter, HeaderInfo, Indexed};
use opendnp3::enums::{EventMode, MasterTaskType, TaskCompletion};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{DefaultMasterApplication, Master, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{
    DatabaseConfig, DefaultCommandHandler, DefaultOutstationApplication, Outstation,
    OutstationConfig,
};
use opendnp3_std::{
    BlockingMasterCommand, BlockingOutstationCommand, blocking_master_channel,
    blocking_outstation_channel, run_blocking_master, run_blocking_outstation,
};

struct SharedSoe {
    binaries: Arc<Mutex<Vec<Indexed<Binary>>>>,
    counters: Arc<Mutex<Vec<Indexed<Counter>>>>,
}

impl SoeHandler for SharedSoe {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.lock().unwrap().extend_from_slice(values);
    }
    fn process_counter(&mut self, _info: HeaderInfo, values: &[Indexed<Counter>]) {
        self.counters.lock().unwrap().extend_from_slice(values);
    }
}

#[test]
fn std_blocking_tcp_master_and_outstation_end_to_end() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let (o_tx, o_rx) = blocking_outstation_channel();
    let o_thread = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut o_link = LinkConfig::new(false);
        o_link.local_addr = 10;
        o_link.remote_addr = 1;
        let o_cfg = OutstationConfig {
            database: DatabaseConfig::all_types(2),
            ..Default::default()
        };
        let mut outstation = Outstation::new(o_cfg, Addresses::new(10, 1));
        outstation
            .database
            .update_binary(0, Binary::new(true), EventMode::Suppress);
        outstation
            .database
            .update_counter(1, Counter::new(4242), EventMode::Suppress);
        let mut cmd = DefaultCommandHandler;
        let mut app = DefaultOutstationApplication;
        run_blocking_outstation(stream, o_link, &mut outstation, &mut cmd, &mut app, &o_rx)
            .unwrap();
    });

    let (m_tx, m_rx) = blocking_master_channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    let binaries = Arc::new(Mutex::new(Vec::new()));
    let counters = Arc::new(Mutex::new(Vec::new()));
    let b_clone = binaries.clone();
    let c_clone = counters.clone();

    let m_thread = thread::spawn(move || {
        let stream = TcpStream::connect(addr).unwrap();
        let mut m_link = LinkConfig::new(true);
        m_link.local_addr = 1;
        m_link.remote_addr = 10;
        let mut master = Master::new(MasterParams::default(), Addresses::new(1, 10));
        let mut soe = SharedSoe {
            binaries: b_clone,
            counters: c_clone,
        };
        let mut app = DefaultMasterApplication;
        run_blocking_master(
            stream,
            m_link,
            &mut master,
            &mut soe,
            &mut app,
            &m_rx,
            &ev_tx,
        )
        .unwrap();
    });

    loop {
        let ev = ev_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if matches!(
            ev,
            MasterEvent::TaskComplete {
                task_type: MasterTaskType::StartupIntegrityPoll,
                result: TaskCompletion::Success,
            }
        ) {
            break;
        }
    }

    assert!(
        binaries
            .lock()
            .unwrap()
            .iter()
            .any(|b| b.index == 0 && b.value.value)
    );
    assert!(
        counters
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.index == 1 && c.value.value == 4242)
    );

    let _ = m_tx.send(BlockingMasterCommand::Shutdown);
    let _ = o_tx.send(BlockingOutstationCommand::Shutdown);
    m_thread.join().unwrap();
    o_thread.join().unwrap();
}
