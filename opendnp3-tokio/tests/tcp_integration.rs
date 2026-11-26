//! End-to-end async integration test: Tokio TCP Master <-> Tokio TCP Outstation.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use opendnp3::app::{
    Analog, AnalogOutputInt32, Binary, ClassField, ControlRelayOutputBlock, HeaderInfo, Indexed,
};
use opendnp3::enums::{
    ChannelState, CommandStatus, EventMode, MasterTaskType, OperateType, OperationType,
    TaskCompletion, TripCloseCode,
};
use opendnp3::link::LinkConfig;
use opendnp3::master::{
    CommandSet, DefaultMasterApplication, MasterEvent, MasterParams, SoeHandler,
};
use opendnp3::outstation::{
    CommandHandler, DatabaseConfig, DefaultOutstationApplication, OutstationConfig,
};
use opendnp3_tokio::{
    RetryStrategy, spawn_tcp_master, spawn_tcp_master_reconnecting, spawn_tcp_outstation,
};
use tokio::net::TcpListener;
use tokio::time::timeout;

#[derive(Default)]
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

#[derive(Default)]
struct SharedCmdHandler {
    crob_operates: Arc<Mutex<Vec<u16>>>,
    ao_operates: Arc<Mutex<Vec<(u16, i32)>>>,
}

impl CommandHandler for SharedCmdHandler {
    fn select_crob(&mut self, _command: &ControlRelayOutputBlock, _index: u16) -> CommandStatus {
        CommandStatus::Success
    }
    fn operate_crob(
        &mut self,
        _command: &ControlRelayOutputBlock,
        index: u16,
        _op_type: OperateType,
    ) -> CommandStatus {
        self.crob_operates.lock().unwrap().push(index);
        CommandStatus::Success
    }
    fn operate_ao_i32(
        &mut self,
        command: &AnalogOutputInt32,
        index: u16,
        _op_type: OperateType,
    ) -> CommandStatus {
        self.ao_operates
            .lock()
            .unwrap()
            .push((index, command.value));
        CommandStatus::Success
    }
}

#[tokio::test]
async fn tcp_master_and_outstation_end_to_end() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Configure Outstation
    let mut o_link = LinkConfig::new(false);
    o_link.local_addr = 10;
    o_link.remote_addr = 1;

    let mut o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(4),
        ..Default::default()
    };
    o_cfg.params.allow_unsolicited = true;

    let crob_log = Arc::new(Mutex::new(Vec::new()));
    let ao_log = Arc::new(Mutex::new(Vec::new()));
    let o_handle = spawn_tcp_outstation(
        listener,
        o_link,
        o_cfg,
        SharedCmdHandler {
            crob_operates: crob_log.clone(),
            ao_operates: ao_log.clone(),
        },
        DefaultOutstationApplication,
    );

    // Seed initial values
    o_handle.update_binary(0, Binary::new(true), EventMode::Suppress);
    o_handle.update_analog(2, Analog::new(123.0), EventMode::Suppress);

    // Configure & connect Master
    let mut m_link = LinkConfig::new(true);
    m_link.local_addr = 1;
    m_link.remote_addr = 10;

    let binaries = Arc::new(Mutex::new(Vec::new()));
    let analogs = Arc::new(Mutex::new(Vec::new()));
    let (m_handle, mut ev_rx) = spawn_tcp_master(
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

    // 1) Wait for startup integrity poll to complete
    timeout(Duration::from_secs(5), async {
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
    })
    .await
    .expect("startup integrity poll timed out");

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
            .any(|a| a.index == 2 && (a.value.value - 123.0).abs() < 1e-6)
    );

    // 2) Send SBO CROB command over TCP
    let mut cs = CommandSet::new();
    cs.add_crob(
        3,
        ControlRelayOutputBlock::new(
            OperationType::PulseOn,
            TripCloseCode::Close,
            false,
            1,
            50,
            50,
            CommandStatus::Success,
        ),
    );
    m_handle.select_and_operate(cs);

    let cmd_res = timeout(Duration::from_secs(5), async {
        while let Some(ev) = ev_rx.recv().await {
            if let MasterEvent::CommandComplete(res) = ev {
                return res;
            }
        }
        panic!("event channel closed");
    })
    .await
    .expect("SBO command timed out");

    assert_eq!(cmd_res.summary, TaskCompletion::Success);
    assert_eq!(cmd_res.points.len(), 1);
    assert_eq!(cmd_res.points[0].status, CommandStatus::Success);
    assert_eq!(*crob_log.lock().unwrap(), vec![3]);

    // 3) Send DirectOperate AnalogOutputInt32 over TCP
    let mut cs2 = CommandSet::new();
    cs2.add_ao_i32(1, AnalogOutputInt32::new(-500));
    m_handle.direct_operate(cs2, false);

    let cmd_res2 = timeout(Duration::from_secs(5), async {
        while let Some(ev) = ev_rx.recv().await {
            if let MasterEvent::CommandComplete(res) = ev {
                return res;
            }
        }
        panic!("event channel closed");
    })
    .await
    .expect("DirectOperate timed out");
    assert_eq!(cmd_res2.summary, TaskCompletion::Success);
    assert_eq!(*ao_log.lock().unwrap(), vec![(1, -500)]);

    // 4) Enable unsolicited, push a live event from OutstationHandle, and verify Master receives it
    m_handle.enable_unsolicited(ClassField::ALL_EVENT_CLASSES);
    timeout(Duration::from_secs(5), async {
        while let Some(ev) = ev_rx.recv().await {
            if matches!(
                ev,
                MasterEvent::TaskComplete {
                    task_type: MasterTaskType::EnableUnsol,
                    result: TaskCompletion::Success,
                }
            ) {
                break;
            }
        }
    })
    .await
    .expect("enable unsolicited timed out");

    o_handle.update_binary(1, Binary::new(true), EventMode::Force);
    timeout(Duration::from_secs(5), async {
        loop {
            if binaries
                .lock()
                .unwrap()
                .iter()
                .any(|b| b.index == 1 && b.value.value)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("unsolicited event delivery timed out");

    m_handle.shutdown();
    o_handle.shutdown();
}

#[tokio::test]
async fn tcp_master_reconnecting_and_periodic_scan() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Start master BEFORE outstation is accepting (tests backoff & reconnect)
    let mut m_link = LinkConfig::new(true);
    m_link.local_addr = 1;
    m_link.remote_addr = 10;

    let (state_tx, mut state_rx) = tokio::sync::mpsc::unbounded_channel();
    let binaries = Arc::new(Mutex::new(Vec::new()));
    let (m_handle, mut ev_rx) = spawn_tcp_master_reconnecting(
        addr,
        m_link,
        MasterParams::default(),
        RetryStrategy::new(Duration::from_millis(20), Duration::from_millis(100)),
        SharedSoe {
            binaries: binaries.clone(),
            analogs: Arc::new(Mutex::new(Vec::new())),
        },
        DefaultMasterApplication,
        Some(state_tx),
    );

    // Register a fast 50ms periodic Class 1 scan
    m_handle.add_class_scan(ClassField::CLASS_1, Duration::from_millis(50));

    // Now start the outstation on the bound listener
    let mut o_link = LinkConfig::new(false);
    o_link.local_addr = 10;
    o_link.remote_addr = 1;
    let o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    let o_handle = spawn_tcp_outstation(
        listener,
        o_link,
        o_cfg,
        SharedCmdHandler::default(),
        DefaultOutstationApplication,
    );

    // Wait for ChannelState::Open
    timeout(Duration::from_secs(5), async {
        while let Some(st) = state_rx.recv().await {
            if st == ChannelState::Open {
                break;
            }
        }
    })
    .await
    .expect("reconnecting master failed to open");

    // Push a Class 1 binary event and verify the periodic Class 1 scan picks it up automatically
    o_handle.update_binary(0, Binary::new(true), EventMode::Force);
    timeout(Duration::from_secs(5), async {
        while let Some(ev) = ev_rx.recv().await {
            if matches!(
                ev,
                MasterEvent::TaskComplete {
                    task_type: MasterTaskType::AutoEventScan,
                    result: TaskCompletion::Success,
                }
            ) && binaries
                .lock()
                .unwrap()
                .iter()
                .any(|b| b.index == 0 && b.value.value)
            {
                break;
            }
        }
    })
    .await
    .expect("periodic Class 1 scan did not deliver event");

    m_handle.shutdown();
    o_handle.shutdown();
}
