//! Ported C++ Master conformance tests:
//! - `TestMaster.cpp` (startup sequence, IIN reactions, non-LAN & LAN time sync, event overflow, link down failure)
//! - `TestMasterAssignClass.cpp`
//! - `TestMasterCommandRequests.cpp` (SBO mismatch, select failure, DirectOperateNR)
//! - `TestMasterMultiCommandRequests.cpp`
//! - Periodic scan scheduler tests

use std::time::Duration;

use opendnp3::Message;
use opendnp3::app::{
    AnalogOutputInt32, ApduHeader, ClassField, ControlRelayOutputBlock, DnpTime, IinBit, IinField,
    Range,
};
use opendnp3::clock::ManualClock;
use opendnp3::enums::{
    CommandPointState, CommandStatus, FunctionCode, GroupVariation, MasterTaskType, OperationType,
    PointClass, TaskCompletion, TimeSyncMode, TripCloseCode,
};
use opendnp3::link::Addresses;
use opendnp3::master::{
    CommandSet, DefaultMasterApplication, Master, MasterApplication, MasterEvent, MasterParams,
    NullSoeHandler,
};
use opendnp3::util::hex::{from_hex, to_hex};

struct MockMasterApp {
    now_dnp: DnpTime,
    received_iins: Vec<IinField>,
    completed: Vec<(MasterTaskType, TaskCompletion)>,
}

impl MockMasterApp {
    fn new(now_ms: u64) -> Self {
        Self {
            now_dnp: DnpTime::new(now_ms),
            received_iins: Vec::new(),
            completed: Vec::new(),
        }
    }
}

impl MasterApplication for MockMasterApp {
    fn on_receive_iin(&mut self, iin: IinField) {
        self.received_iins.push(iin);
    }
    fn on_task_complete(&mut self, task_type: MasterTaskType, result: TaskCompletion) {
        self.completed.push((task_type, result));
    }
    fn now_dnp_time(&self) -> DnpTime {
        self.now_dnp
    }
}

fn pop_tx_hex(
    m: &mut Master,
    clock: &ManualClock,
    app: &mut dyn MasterApplication,
) -> Option<String> {
    let msg = m.pop_transmit()?;
    let hex = to_hex(&msg.payload);
    m.on_tx_ready(clock.now(), app);
    Some(hex)
}

fn feed_rsp(
    m: &mut Master,
    clock: &ManualClock,
    hex: &str,
    soe: &mut NullSoeHandler,
    app: &mut dyn MasterApplication,
) {
    let msg = Message::new(Addresses::new(10, 1), from_hex(hex).unwrap());
    m.on_receive(&msg, clock.now(), soe, app);
}

#[test]
fn master_startup_disable_unsol_integrity_enable_unsol_sequence() {
    let clock = ManualClock::new();
    let mut m = Master::new(MasterParams::default(), Addresses::new(1, 10));
    let mut app = MockMasterApp::new(0);
    let mut soe = NullSoeHandler;

    m.on_lower_layer_up(clock.now(), &mut app);

    // 1) Disable Unsolicited (FC=0x15, seq=1 -> C1 15 3C 02 06 3C 03 06 3C 04 06)
    let tx1 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx1, "C1 15 3C 02 06 3C 03 06 3C 04 06");
    feed_rsp(&mut m, &clock, "C1 81 00 00", &mut soe, &mut app);

    // 2) Startup Integrity Poll (FC=0x01, seq=2)
    let tx2 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx2, "C2 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06");
    feed_rsp(&mut m, &clock, "C2 81 00 00", &mut soe, &mut app);

    // 3) Enable Unsolicited (FC=0x14, seq=3)
    let tx3 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx3, "C3 14 3C 02 06 3C 03 06 3C 04 06");
    feed_rsp(&mut m, &clock, "C3 81 00 00", &mut soe, &mut app);

    assert_eq!(
        app.completed,
        vec![
            (MasterTaskType::DisableUnsol, TaskCompletion::Success),
            (
                MasterTaskType::StartupIntegrityPoll,
                TaskCompletion::Success
            ),
            (MasterTaskType::EnableUnsol, TaskCompletion::Success),
        ]
    );
}

#[test]
fn master_non_lan_two_step_time_sync() {
    let mut clock = ManualClock::new();
    let params = MasterParams {
        time_sync_mode: TimeSyncMode::NonLan,
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::NONE,
        ..Default::default()
    };
    let mut m = Master::new(params, Addresses::new(1, 10));
    let mut app = MockMasterApp::new(10_000);
    let mut soe = NullSoeHandler;

    m.on_lower_layer_up(clock.now(), &mut app);
    // Unsolicited response with NeedTime (IIN1 bit 4 = 0x10)
    feed_rsp(&mut m, &clock, "F0 82 10 00", &mut soe, &mut app);
    // First TX is unsolicited confirm
    let confirm = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(confirm, "D0 00");

    // Step 1: DELAY_MEASURE (FC=0x17, seq=1)
    let tx1 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx1, "C1 17");

    // Simulate 100ms RTT with 20ms outstation turnaround -> 40ms one-way propagation delay
    clock.advance(Duration::from_millis(100));
    // Response with G52V2 (fine delay = 20ms = 0x0014)
    feed_rsp(
        &mut m,
        &clock,
        "C1 81 10 00 34 02 07 01 14 00",
        &mut soe,
        &mut app,
    );

    // Step 2: WRITE G50V1 with time = 10_000 + 40 = 10_040 (0x2738 -> 38 27 00 00 00 00)
    let tx2 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx2, "C2 02 32 01 07 01 38 27 00 00 00 00");
    feed_rsp(&mut m, &clock, "C2 81 00 00", &mut soe, &mut app);

    assert!(
        app.completed
            .contains(&(MasterTaskType::NonLanTimeSync, TaskCompletion::Success))
    );
}

#[test]
fn master_lan_two_step_time_sync() {
    let clock = ManualClock::new();
    let params = MasterParams {
        time_sync_mode: TimeSyncMode::Lan,
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::NONE,
        ..Default::default()
    };
    let mut m = Master::new(params, Addresses::new(1, 10));
    let mut app = MockMasterApp::new(20_000);
    let mut soe = NullSoeHandler;

    m.on_lower_layer_up(clock.now(), &mut app);
    feed_rsp(&mut m, &clock, "D0 82 10 00", &mut soe, &mut app);

    // Step 1: RECORD_CURRENT_TIME (FC=0x18, seq=1)
    let tx1 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx1, "C1 18");
    feed_rsp(&mut m, &clock, "C1 81 10 00", &mut soe, &mut app);

    // Step 2: WRITE G50V3 (Last Recorded Time = 20_000 = 0x4E20 -> 20 4E 00 00 00 00)
    let tx2 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx2, "C2 02 32 03 07 01 20 4E 00 00 00 00");
    feed_rsp(&mut m, &clock, "C2 81 00 00", &mut soe, &mut app);

    assert!(
        app.completed
            .contains(&(MasterTaskType::LanTimeSync, TaskCompletion::Success))
    );
}

#[test]
fn master_assign_class_and_periodic_scans() {
    let mut clock = ManualClock::new();
    let params = MasterParams {
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::NONE,
        ..Default::default()
    };
    let mut m = Master::new(params, Addresses::new(1, 10));
    let mut app = DefaultMasterApplication;
    let mut soe = NullSoeHandler;

    m.add_class_scan(ClassField::CLASS_1, Duration::from_secs(5), clock.now());
    m.on_lower_layer_up(clock.now(), &mut app);

    // AssignClassAll
    m.assign_class_all(
        PointClass::Class2,
        GroupVariation::Group1Var0,
        clock.now(),
        &mut app,
    );
    let tx = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx, "C1 16 3C 03 06 01 00 06");
    feed_rsp(&mut m, &clock, "C1 81 00 00", &mut soe, &mut app);

    // Periodic scan fires after 5s
    assert!(m.next_wakeup().is_some());
    clock.advance(Duration::from_secs(5));
    m.on_timeout(clock.now(), &mut app);
    let scan_tx = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(scan_tx, "C2 01 3C 02 06");
    feed_rsp(&mut m, &clock, "C2 81 00 00", &mut soe, &mut app);

    // Fires again after another 5s
    clock.advance(Duration::from_secs(5));
    m.on_timeout(clock.now(), &mut app);
    let scan_tx2 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(scan_tx2, "C3 01 3C 02 06");
}

#[test]
fn master_sbo_select_fail_and_direct_operate_no_ack() {
    let clock = ManualClock::new();
    let params = MasterParams {
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::NONE,
        ..Default::default()
    };
    let mut m = Master::new(params, Addresses::new(1, 10));
    let mut app = DefaultMasterApplication;
    let mut soe = NullSoeHandler;
    m.on_lower_layer_up(clock.now(), &mut app);

    // SBO where outstation returns CommandStatus::Local (7) on SELECT -> no OPERATE sent!
    let mut cs = CommandSet::new();
    cs.add_crob(
        0,
        ControlRelayOutputBlock::new(
            OperationType::LatchOn,
            TripCloseCode::Nul,
            false,
            1,
            100,
            100,
            CommandStatus::Success,
        ),
    );
    m.select_and_operate(cs.clone(), clock.now(), &mut app);
    let sel_tx = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(
        ApduHeader::parse(&from_hex(&sel_tx).unwrap())
            .unwrap()
            .function,
        FunctionCode::Select
    );

    // Reply with status = Local (0x07) instead of Success (0x00)
    let bad_rsp = from_hex(&sel_tx).unwrap();
    let payload_after_hdr = bad_rsp[2..].to_vec();
    let mut rsp_bytes = vec![0xC1, 0x81, 0x00, 0x00];
    rsp_bytes.extend_from_slice(&payload_after_hdr);
    *rsp_bytes.last_mut().unwrap() = CommandStatus::Local.to_u8();
    feed_rsp(&mut m, &clock, &to_hex(&rsp_bytes), &mut soe, &mut app);

    // No OPERATE should be queued
    assert!(m.pop_transmit().is_none());
    let events = m.drain_events();
    assert!(events.iter().any(|e| matches!(
        e,
        MasterEvent::CommandComplete(r)
            if r.points.len() == 1
                && r.points[0].state == CommandPointState::SelectFail
                && r.points[0].status == CommandStatus::Local
    )));

    // DirectOperateNoAck completes immediately on on_tx_ready without waiting for response
    let mut cs2 = CommandSet::new();
    cs2.add_ao_i32(2, AnalogOutputInt32::new(55));
    m.direct_operate(cs2, true, clock.now(), &mut app);
    let do_tx = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(
        ApduHeader::parse(&from_hex(&do_tx).unwrap())
            .unwrap()
            .function,
        FunctionCode::DirectOperateNr
    );
    let events2 = m.drain_events();
    assert!(events2.iter().any(|e| matches!(
        e,
        MasterEvent::CommandComplete(r) if r.summary == TaskCompletion::Success
    )));
}

#[test]
fn master_lower_layer_down_fails_pending_tasks_and_iin_event_overflow_triggers_integrity() {
    let clock = ManualClock::new();
    let params = MasterParams {
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::ALL_CLASSES,
        unsol_class_mask: ClassField::NONE,
        integrity_on_event_overflow_iin: true,
        event_scan_on_events_available_class_mask: ClassField::CLASS_1,
        ..Default::default()
    };
    let mut m = Master::new(params, Addresses::new(1, 10));
    let mut app = MockMasterApp::new(0);
    let mut soe = NullSoeHandler;
    m.on_lower_layer_up(clock.now(), &mut app);

    // Startup integrity poll
    let _ = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    // Respond with EventBufferOverflow (IIN2 bit 3 = 0x08) + Class1Events (IIN1 bit 1 = 0x02)
    let mut iin = IinField::empty();
    iin.set_bit(IinBit::EventBufferOverflow);
    iin.set_bit(IinBit::Class1Events);
    feed_rsp(
        &mut m,
        &clock,
        &format!("C1 81 {:02X} {:02X}", iin.lsb, iin.msb),
        &mut soe,
        &mut app,
    );

    // Master should immediately queue another integrity poll because of EventBufferOverflow
    let tx2 = pop_tx_hex(&mut m, &clock, &mut app).unwrap();
    assert_eq!(tx2, "C2 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06");

    // Drop link mid-request -> active + queued tasks fail with FailureNoComms
    m.scan_range(
        GroupVariation::Group1Var2,
        Range::new(0, 3),
        clock.now(),
        &mut app,
    );
    m.on_lower_layer_down(&mut app);
    assert!(
        app.completed
            .iter()
            .filter(|(_, r)| *r == TaskCompletion::FailureNoComms)
            .count()
            >= 2
    );
}
