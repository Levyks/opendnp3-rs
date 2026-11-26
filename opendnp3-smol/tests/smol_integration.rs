//! End-to-end TCP integration test running Master <-> Outstation on `smol`.

use std::sync::{Arc, Mutex};

use opendnp3::app::{Analog, Binary, HeaderInfo, Indexed};
use opendnp3::enums::{EventMode, MasterTaskType, TaskCompletion};
use opendnp3::link::LinkConfig;
use opendnp3::master::{DefaultMasterApplication, MasterEvent, MasterParams, SoeHandler};
use opendnp3::outstation::{
    DatabaseConfig, DefaultCommandHandler, DefaultOutstationApplication, OutstationConfig,
};
use opendnp3_smol::{spawn_tcp_master, spawn_tcp_outstation};
use smol::net::TcpListener;

struct SharedSoe {
    binaries: Arc<Mutex<Vec<Indexed<Binary>>>>,
    analogs: Arc<Mutex<Vec<Indexed<Analog>>>>,
}

impl SoeHandler for SharedSoe {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.lock().unwrap().extend_from_slice(values);
    }
    fn process_analog(&mut self, _info: HeaderInfo, values: &[Indexed<Analog>]) {
        self.analogs.lock().unwrap().extend_from_slice(values);
    }
}

#[test]
fn smol_tcp_master_and_outstation_end_to_end() {
    smol::block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let mut o_link = LinkConfig::new(false);
        o_link.local_addr = 10;
        o_link.remote_addr = 1;
        let o_cfg = OutstationConfig {
            database: DatabaseConfig::all_types(3),
            ..Default::default()
        };
        let o_handle = spawn_tcp_outstation(
            listener,
            o_link,
            o_cfg,
            DefaultCommandHandler,
            DefaultOutstationApplication,
        );
        o_handle.update_binary(0, Binary::new(true), EventMode::Suppress);
        o_handle.update_analog(2, Analog::new(88.0), EventMode::Suppress);

        let mut m_link = LinkConfig::new(true);
        m_link.local_addr = 1;
        m_link.remote_addr = 10;
        let binaries = Arc::new(Mutex::new(Vec::new()));
        let analogs = Arc::new(Mutex::new(Vec::new()));
        let (m_handle, ev_rx) = spawn_tcp_master(
            addr,
            m_link,
            MasterParams::default(),
            SharedSoe {
                binaries: binaries.clone(),
                analogs: analogs.clone(),
            },
            DefaultMasterApplication,
        )
        .await
        .unwrap();

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
                .lock()
                .unwrap()
                .iter()
                .any(|b| b.index == 0 && b.value.value)
        );
        assert!(
            analogs
                .lock()
                .unwrap()
                .iter()
                .any(|a| a.index == 2 && (a.value.value - 88.0).abs() < 1e-6)
        );

        m_handle.shutdown();
        o_handle.shutdown();
    });
}
