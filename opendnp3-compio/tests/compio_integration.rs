//! End-to-end TCP integration test running Master <-> Outstation on `compio`.

use std::cell::RefCell;
use std::rc::Rc;

use compio::net::{TcpListener, TcpStream};
use opendnp3::app::{Analog, Binary, HeaderInfo, Indexed};
use opendnp3::driver::{master_channel, outstation_channel, unbounded};
use opendnp3::enums::{EventMode, MasterTaskType, TaskCompletion};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{DefaultMasterApplication, Master, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{
    DatabaseConfig, DefaultCommandHandler, DefaultOutstationApplication, Outstation,
    OutstationConfig,
};
use opendnp3_compio::{run_compio_master, run_compio_outstation};

struct LocalSoe {
    binaries: Rc<RefCell<Vec<Indexed<Binary>>>>,
    analogs: Rc<RefCell<Vec<Indexed<Analog>>>>,
}

impl SoeHandler for LocalSoe {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.borrow_mut().extend_from_slice(values);
    }
    fn process_analog(&mut self, _info: HeaderInfo, values: &[Indexed<Analog>]) {
        self.analogs.borrow_mut().extend_from_slice(values);
    }
}

#[test]
fn compio_tcp_master_and_outstation_end_to_end() {
    let rt = compio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (o_handle, o_cmd_rx) = outstation_channel();
        let o_task = compio::runtime::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut o_link = LinkConfig::new(false);
            o_link.local_addr = 10;
            o_link.remote_addr = 1;
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
                .update_analog(1, Analog::new(555.0), EventMode::Suppress);
            let mut cmd = DefaultCommandHandler;
            let mut app = DefaultOutstationApplication;
            run_compio_outstation(
                stream,
                o_link,
                &mut outstation,
                &mut cmd,
                &mut app,
                &o_cmd_rx,
            )
            .await
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let (m_handle, m_cmd_rx) = master_channel();
        let (ev_tx, ev_rx) = unbounded();
        let binaries = Rc::new(RefCell::new(Vec::new()));
        let analogs = Rc::new(RefCell::new(Vec::new()));
        let b_clone = binaries.clone();
        let a_clone = analogs.clone();

        let m_task = compio::runtime::spawn(async move {
            let mut m_link = LinkConfig::new(true);
            m_link.local_addr = 1;
            m_link.remote_addr = 10;
            let mut master = Master::new(MasterParams::default(), Addresses::new(1, 10));
            let mut soe = LocalSoe {
                binaries: b_clone,
                analogs: a_clone,
            };
            let mut app = DefaultMasterApplication;
            run_compio_master(
                stream,
                m_link,
                &mut master,
                &mut soe,
                &mut app,
                &m_cmd_rx,
                &ev_tx,
            )
            .await
        });

        while let Some(ev) = ev_rx.recv().await {
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
                .borrow()
                .iter()
                .any(|b| b.index == 0 && b.value.value)
        );
        assert!(
            analogs
                .borrow()
                .iter()
                .any(|a| a.index == 1 && (a.value.value - 555.0).abs() < 1e-6)
        );

        m_handle.shutdown();
        o_handle.shutdown();
        let _ = m_task.await;
        let _ = o_task.await;
    });
}
