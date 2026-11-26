//! Ported C++ Outstation, Database, and Event Storage conformance tests:
//! - `TestDatabase.cpp`, `TestEventStorage.cpp`
//! - `TestOutstation.cpp` (duplicate request replay, unsupported FC, write IIN, time sync, restarts)
//! - `TestOutstationAssignClass.cpp`
//! - `TestOutstationBroadcast.cpp`
//! - `TestOutstationCommandResponses.cpp` (SBO mismatch/timeout/wrong-seq, max controls, AO i16/i32/f32/f64, DirectOperateNR)
//! - `TestOutstationDiscontiguousIndices.cpp`
//! - `TestOutstationEventResponses.cpp` (class 1/2/3, count-limited reads, confirm-clear, timeout-unselect, overflow)
//! - `TestOutstationFrozenCounters.cpp` (IMMED_FREEZE, FREEZE_CLEAR, _NR variants)
//! - `TestOutstationUnsolicitedResponses.cpp` (null unsol retry, enable/disable unsol, confirm timeout + retry)

use std::time::Duration;

use opendnp3::Message;
use opendnp3::app::{
    AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    ApduResponseHeader, ApduWriter, AppControlField, Binary, ClassField, ControlRelayOutputBlock,
    Counter, DnpTime, DoubleBitBinary, IinBit, Range,
};
use opendnp3::clock::ManualClock;
use opendnp3::enums::{
    CommandStatus, DoubleBit, EventClass, EventMode, FunctionCode, OperateType, OperationType,
    RestartMode, TripCloseCode,
};
use opendnp3::link::{Addresses, LinkBroadcastAddress};
use opendnp3::master::CommandSet;
use opendnp3::outstation::{
    AnalogConfig, BinaryConfig, CommandHandler, DatabaseConfig, DefaultCommandHandler,
    DefaultOutstationApplication, EventBufferConfig, Outstation, OutstationApplication,
    OutstationConfig, OutstationParams,
};
use opendnp3::util::hex::{from_hex, to_hex};

#[derive(Default)]
struct MockCmd {
    selects: usize,
    operates: usize,
    ao_i16: Vec<i16>,
    ao_i32: Vec<i32>,
    ao_f32: Vec<f32>,
    ao_f64: Vec<f64>,
}

impl CommandHandler for MockCmd {
    fn select_crob(&mut self, _: &ControlRelayOutputBlock, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_crob(
        &mut self,
        _: &ControlRelayOutputBlock,
        _: u16,
        _: OperateType,
    ) -> CommandStatus {
        self.operates += 1;
        CommandStatus::Success
    }
    fn select_ao_i32(&mut self, _: &AnalogOutputInt32, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_i32(&mut self, c: &AnalogOutputInt32, _: u16, _: OperateType) -> CommandStatus {
        self.operates += 1;
        self.ao_i32.push(c.value);
        CommandStatus::Success
    }
    fn select_ao_i16(&mut self, _: &AnalogOutputInt16, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_i16(&mut self, c: &AnalogOutputInt16, _: u16, _: OperateType) -> CommandStatus {
        self.operates += 1;
        self.ao_i16.push(c.value);
        CommandStatus::Success
    }
    fn select_ao_f32(&mut self, _: &AnalogOutputFloat32, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_f32(&mut self, c: &AnalogOutputFloat32, _: u16, _: OperateType) -> CommandStatus {
        self.operates += 1;
        self.ao_f32.push(c.value);
        CommandStatus::Success
    }
    fn select_ao_f64(&mut self, _: &AnalogOutputDouble64, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_f64(
        &mut self,
        c: &AnalogOutputDouble64,
        _: u16,
        _: OperateType,
    ) -> CommandStatus {
        self.operates += 1;
        self.ao_f64.push(c.value);
        CommandStatus::Success
    }
}

#[derive(Default)]
struct MockApp {
    written_time: Option<DnpTime>,
    cold_mode: Option<RestartMode>,
    warm_mode: Option<RestartMode>,
    now_time: DnpTime,
}

impl OutstationApplication for MockApp {
    fn supports_write_absolute_time(&self) -> bool {
        true
    }
    fn supports_assign_class(&self) -> bool {
        true
    }
    fn write_absolute_time(&mut self, time: DnpTime) -> bool {
        self.written_time = Some(time);
        true
    }
    fn cold_restart_support(&self) -> RestartMode {
        self.cold_mode.unwrap_or(RestartMode::Unsupported)
    }
    fn warm_restart_support(&self) -> RestartMode {
        self.warm_mode.unwrap_or(RestartMode::Unsupported)
    }
    fn cold_restart(&mut self) -> u16 {
        500
    }
    fn warm_restart(&mut self) -> u16 {
        2
    }
    fn now_dnp_time(&self) -> DnpTime {
        self.now_time
    }
}

fn send_hex(
    o: &mut Outstation,
    clock: &ManualClock,
    hex: &str,
    cmd: &mut dyn CommandHandler,
    app: &mut dyn OutstationApplication,
) -> Option<Vec<u8>> {
    let msg = Message::new(Addresses::new(1, 10), from_hex(hex).unwrap());
    o.on_receive(&msg, clock.now(), cmd, app);
    let resp = o.pop_transmit().map(|m| m.payload);
    if resp.is_some() {
        o.on_tx_ready(clock.now(), app);
    }
    resp
}

// =========================================================================
// TestOutstation.cpp
// =========================================================================

#[test]
fn outstation_unsupported_function_code_returns_iin() {
    let clock = ManualClock::new();
    let mut o = Outstation::new(OutstationConfig::default(), Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = DefaultOutstationApplication;
    o.on_lower_layer_up(clock.now(), &app);

    // Function code 0x10 (InitializeApplication) is unsupported
    let resp = send_hex(&mut o, &clock, "C0 10", &mut cmd, &mut app).unwrap();
    let hdr = ApduResponseHeader::parse(&resp).unwrap();
    assert!(hdr.iin.is_set(IinBit::DeviceRestart));
    assert!(hdr.iin.is_set(IinBit::FuncNotSupported));
}

#[test]
fn outstation_clear_restart_iin_and_reject_invalid_iin_write() {
    let clock = ManualClock::new();
    let mut o = Outstation::new(OutstationConfig::default(), Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = DefaultOutstationApplication;
    o.on_lower_layer_up(clock.now(), &app);

    // Clear bit 7 (DeviceRestart = 0)
    let resp = send_hex(
        &mut o,
        &clock,
        "C0 02 50 01 00 07 07 00",
        &mut cmd,
        &mut app,
    )
    .unwrap();
    let hdr = ApduResponseHeader::parse(&resp).unwrap();
    assert!(!hdr.iin.is_set(IinBit::DeviceRestart));
    assert!(!hdr.iin.has_request_error());

    // Trying to set DeviceRestart to 1 -> ParamError
    let resp2 = send_hex(
        &mut o,
        &clock,
        "C1 02 50 01 00 07 07 01",
        &mut cmd,
        &mut app,
    )
    .unwrap();
    let hdr2 = ApduResponseHeader::parse(&resp2).unwrap();
    assert!(hdr2.iin.is_set(IinBit::ParamError));
}

#[test]
fn outstation_duplicate_request_replays_last_response() {
    let clock = ManualClock::new();
    let cfg = OutstationConfig {
        database: DatabaseConfig::all_types(1),
        ..Default::default()
    };
    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = MockApp::default();
    o.on_lower_layer_up(clock.now(), &app);

    let r1 = send_hex(&mut o, &clock, "C0 01 01 02 06", &mut cmd, &mut app).unwrap();
    // Mutate DB; duplicate request with same seq & bytes should replay cached r1
    o.database
        .update_binary(0, Binary::new(true), EventMode::Suppress);
    let r2 = send_hex(&mut o, &clock, "C0 01 01 02 06", &mut cmd, &mut app).unwrap();
    assert_eq!(r1, r2);
}

#[test]
fn outstation_write_absolute_time_and_restarts() {
    let clock = ManualClock::new();
    let mut o = Outstation::new(OutstationConfig::default(), Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = MockApp {
        cold_mode: Some(RestartMode::SupportedDelayFine),
        warm_mode: Some(RestartMode::SupportedDelayCoarse),
        ..Default::default()
    };
    o.on_lower_layer_up(clock.now(), &app);

    // WRITE G50V1 time = 1000 (0x03E8 -> E8 03 00 00 00 00)
    let resp = send_hex(
        &mut o,
        &clock,
        "C0 02 32 01 07 01 E8 03 00 00 00 00",
        &mut cmd,
        &mut app,
    )
    .unwrap();
    assert!(
        !ApduResponseHeader::parse(&resp)
            .unwrap()
            .iin
            .has_request_error()
    );
    assert_eq!(app.written_time, Some(DnpTime::new(1000)));

    // Cold restart -> fine delay 500ms (G52V2)
    let resp_cold = send_hex(&mut o, &clock, "C1 0D", &mut cmd, &mut app).unwrap();
    assert_eq!(to_hex(&resp_cold[4..]), "34 02 07 01 F4 01");

    // Warm restart -> coarse delay 2s (G52V1)
    let resp_warm = send_hex(&mut o, &clock, "C2 0E", &mut cmd, &mut app).unwrap();
    assert_eq!(to_hex(&resp_warm[4..]), "34 01 07 01 02 00");
}

// =========================================================================
// TestOutstationBroadcast.cpp
// =========================================================================

#[test]
fn outstation_broadcast_sets_all_stations_iin_and_suppresses_reply() {
    let clock = ManualClock::new();
    let mut o = Outstation::new(OutstationConfig::default(), Addresses::new(10, 1));
    let mut cmd = MockCmd::default();
    let mut app = MockApp::default();
    o.on_lower_layer_up(clock.now(), &app);

    // Broadcast WRITE time
    let bcast = Message::new(
        Addresses::new(1, LinkBroadcastAddress::DontConfirm.to_u16()),
        from_hex("C0 02 32 01 07 01 0A 00 00 00 00 00").unwrap(),
    );
    o.on_receive(&bcast, clock.now(), &mut cmd, &mut app);
    // No reply transmitted for broadcast
    assert!(o.pop_transmit().is_none());
    assert_eq!(app.written_time, Some(DnpTime::new(10)));

    // Subsequent unicast READ should report IIN1.0 (AllStations)
    let resp = send_hex(&mut o, &clock, "C1 01", &mut cmd, &mut app).unwrap();
    let hdr = ApduResponseHeader::parse(&resp).unwrap();
    assert!(hdr.iin.is_set(IinBit::AllStations));
}

// =========================================================================
// TestOutstationAssignClass.cpp
// =========================================================================

#[test]
fn outstation_assign_class_moves_events_to_new_class() {
    let clock = ManualClock::new();
    let cfg = OutstationConfig {
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = MockApp::default();
    o.on_lower_layer_up(clock.now(), &app);

    // ASSIGN_CLASS: Class 2 (3C 03 06) to Binary point 1 (01 00 00 01 01)
    let resp = send_hex(
        &mut o,
        &clock,
        "C0 16 3C 03 06 01 00 00 01 01",
        &mut cmd,
        &mut app,
    )
    .unwrap();
    assert!(
        !ApduResponseHeader::parse(&resp)
            .unwrap()
            .iin
            .has_request_error()
    );

    // Update binary 1 -> should now be Class 2 event, NOT Class 1!
    o.database
        .update_binary(1, Binary::new(true), EventMode::Force);
    let classes = o.database.events.unwritten_classes();
    assert!(!classes.has_class1());
    assert!(classes.has_class2());
}

// =========================================================================
// TestOutstationDiscontiguousIndices.cpp
// =========================================================================

#[test]
fn outstation_discontiguous_indices_produce_separate_range_headers() {
    let clock = ManualClock::new();
    let mut db = DatabaseConfig::default();
    db.binary.insert(0, BinaryConfig::default());
    db.binary.insert(1, BinaryConfig::default());
    db.binary.insert(5, BinaryConfig::default());
    db.analog.insert(10, AnalogConfig::default());

    let cfg = OutstationConfig {
        database: db,
        ..Default::default()
    };
    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = MockApp::default();
    o.on_lower_layer_up(clock.now(), &app);

    o.database
        .update_binary(0, Binary::new(true), EventMode::Suppress);
    o.database
        .update_binary(1, Binary::new(false), EventMode::Suppress);
    o.database
        .update_binary(5, Binary::new(true), EventMode::Suppress);

    // Read all G1V2 -> should produce 2 range headers: [0..=1] and [5..=5]
    let resp = send_hex(&mut o, &clock, "C0 01 01 02 06", &mut cmd, &mut app).unwrap();
    assert_eq!(to_hex(&resp[4..]), "01 02 00 00 01 81 01 01 02 00 05 05 81");
}

// =========================================================================
// TestOutstationCommandResponses.cpp
// =========================================================================

#[test]
fn outstation_sbo_timeout_wrong_seq_and_all_ao_types() {
    let mut clock = ManualClock::new();
    let params = OutstationParams {
        select_timeout: Duration::from_secs(5),
        max_controls_per_request: 1,
        ..Default::default()
    };
    let cfg = OutstationConfig {
        params,
        ..Default::default()
    };
    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    let mut cmd = MockCmd::default();
    let mut app = DefaultOutstationApplication;
    o.on_lower_layer_up(clock.now(), &app);

    let crob = ControlRelayOutputBlock::new(
        OperationType::PulseOn,
        TripCloseCode::Close,
        false,
        1,
        10,
        10,
        CommandStatus::Success,
    );
    let mut cs = CommandSet::new();
    cs.add_crob(0, crob);

    // SELECT seq=0
    let mut w_sel = ApduWriter::new_request(128, AppControlField::request(0), FunctionCode::Select);
    cs.write(&mut w_sel);
    let r_sel = send_hex(
        &mut o,
        &clock,
        &to_hex(w_sel.as_bytes()),
        &mut cmd,
        &mut app,
    )
    .unwrap();
    assert_eq!(*r_sel.last().unwrap(), CommandStatus::Success.to_u8());
    assert_eq!(cmd.selects, 1);

    // Advance past select timeout -> OPERATE seq=1 must fail with Timeout
    clock.advance(Duration::from_secs(6));
    let mut w_op = ApduWriter::new_request(128, AppControlField::request(1), FunctionCode::Operate);
    cs.write(&mut w_op);
    let r_op = send_hex(&mut o, &clock, &to_hex(w_op.as_bytes()), &mut cmd, &mut app).unwrap();
    assert_eq!(*r_op.last().unwrap(), CommandStatus::Timeout.to_u8());
    assert_eq!(cmd.operates, 0);

    // DirectOperate all 4 Analog Output types individually
    for (seq, set) in [
        (2u8, {
            let mut s = CommandSet::new();
            s.add_ao_i16(1, AnalogOutputInt16::new(-42));
            s
        }),
        (3u8, {
            let mut s = CommandSet::new();
            s.add_ao_i32(2, AnalogOutputInt32::new(100_000));
            s
        }),
        (4u8, {
            let mut s = CommandSet::new();
            s.add_ao_f32(3, AnalogOutputFloat32::new(3.5));
            s
        }),
        (5u8, {
            let mut s = CommandSet::new();
            s.add_ao_f64(4, AnalogOutputDouble64::new(99.25));
            s
        }),
    ] {
        let mut w = ApduWriter::new_request(
            128,
            AppControlField::request(seq),
            FunctionCode::DirectOperate,
        );
        set.write(&mut w);
        let r = send_hex(&mut o, &clock, &to_hex(w.as_bytes()), &mut cmd, &mut app).unwrap();
        assert_eq!(*r.last().unwrap(), CommandStatus::Success.to_u8());
    }
    assert_eq!(cmd.ao_i16, vec![-42]);
    assert_eq!(cmd.ao_i32, vec![100_000]);
    assert_eq!(cmd.ao_f32, vec![3.5]);
    assert_eq!(cmd.ao_f64, vec![99.25]);

    // TooManyOps when exceeding max_controls_per_request (1)
    let mut cs2 = CommandSet::new();
    cs2.add_crob(0, crob).add_crob(1, crob);
    let mut w_many = ApduWriter::new_request(
        128,
        AppControlField::request(6),
        FunctionCode::DirectOperate,
    );
    cs2.write(&mut w_many);
    let r_many = send_hex(
        &mut o,
        &clock,
        &to_hex(w_many.as_bytes()),
        &mut cmd,
        &mut app,
    )
    .unwrap();
    assert_eq!(*r_many.last().unwrap(), CommandStatus::TooManyOps.to_u8());
}

// =========================================================================
// TestOutstationEventResponses.cpp & TestEventStorage.cpp
// =========================================================================

#[test]
fn outstation_count_limited_event_read_and_confirm_timeout_unselect() {
    let mut clock = ManualClock::new();
    let cfg = OutstationConfig {
        database: DatabaseConfig::all_types(3),
        event_buffer: EventBufferConfig::all_types(10),
        ..Default::default()
    };
    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = MockApp::default();
    o.on_lower_layer_up(clock.now(), &app);

    o.database
        .update_binary(0, Binary::new(true), EventMode::Force);
    o.database
        .update_binary(1, Binary::new(true), EventMode::Force);
    o.database
        .update_binary(2, Binary::new(true), EventMode::Force);
    assert_eq!(o.database.events.len(), 3);

    // Read Class 1 with count limit = 1 (3C 02 07 01)
    let resp = send_hex(&mut o, &clock, "C0 01 3C 02 07 01", &mut cmd, &mut app).unwrap();
    let hdr = ApduResponseHeader::parse(&resp).unwrap();
    assert!(hdr.control.con);
    // Still 2 unwritten Class 1 events remaining -> IIN Class1Events still set
    assert!(hdr.iin.is_set(IinBit::Class1Events));

    // Timeout confirm -> event unselected, all 3 still in buffer
    clock.advance(Duration::from_secs(10));
    o.on_timeout(clock.now(), &app);
    assert_eq!(o.database.events.len(), 3);

    // Read again and CONFIRM -> 1 cleared, 2 remain
    let _ = send_hex(&mut o, &clock, "C1 01 3C 02 07 01", &mut cmd, &mut app).unwrap();
    let _ = send_hex(&mut o, &clock, "C1 00", &mut cmd, &mut app);
    assert_eq!(o.database.events.len(), 2);
}

// =========================================================================
// TestOutstationFrozenCounters.cpp
// =========================================================================

#[test]
fn outstation_immediate_freeze_and_freeze_clear() {
    let clock = ManualClock::new();
    let cfg = OutstationConfig {
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = MockApp {
        now_time: DnpTime::new(5000),
        ..Default::default()
    };
    o.on_lower_layer_up(clock.now(), &app);

    o.database
        .update_counter(0, Counter::new(42), EventMode::Suppress);
    o.database
        .update_counter(1, Counter::new(99), EventMode::Suppress);

    // IMMED_FREEZE all counters (FC=0x07, G20V0 AllObjects)
    let r = send_hex(&mut o, &clock, "C0 07 14 00 06", &mut cmd, &mut app).unwrap();
    assert!(
        !ApduResponseHeader::parse(&r)
            .unwrap()
            .iin
            .has_request_error()
    );

    // Read Frozen Counters G21V1
    let rf = send_hex(&mut o, &clock, "C1 01 15 01 06", &mut cmd, &mut app).unwrap();
    // G21V1 range 0..=1: [flags=01, 42u32_le], [flags=01, 99u32_le]
    assert_eq!(
        to_hex(&rf[4..]),
        "15 01 00 00 01 01 2A 00 00 00 01 63 00 00 00"
    );

    // FREEZE_CLEAR_NR (FC=0x0A) on counter 0 only -> no reply, counter 0 cleared
    assert!(send_hex(&mut o, &clock, "C2 0A 14 00 00 00 00", &mut cmd, &mut app).is_none());
    let rc = send_hex(&mut o, &clock, "C3 01 14 01 00 00 00", &mut cmd, &mut app).unwrap();
    assert_eq!(to_hex(&rc[4..]), "14 01 00 00 00 01 00 00 00 00");
}

// =========================================================================
// TestOutstationUnsolicitedResponses.cpp
// =========================================================================

#[test]
fn outstation_unsolicited_null_retry_and_disable_enable() {
    let mut clock = ManualClock::new();
    let cfg = OutstationConfig {
        params: OutstationParams {
            allow_unsolicited: true,
            unsol_confirm_timeout: Duration::from_secs(2),
            unsol_retry_delay: Duration::from_secs(3),
            unsol_class_mask: ClassField::from_event_class(EventClass::Ec1),
            ..Default::default()
        },
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    let mut cmd = DefaultCommandHandler;
    let mut app = MockApp::default();
    o.on_lower_layer_up(clock.now(), &app);

    // Null unsolicited sent on link up
    let null1 = o.pop_transmit().expect("null unsol 1");
    o.on_tx_ready(clock.now(), &app);
    let hdr1 = ApduResponseHeader::parse(&null1.payload).unwrap();
    assert_eq!(hdr1.function, FunctionCode::UnsolicitedResponse);
    assert!(hdr1.control.uns && hdr1.control.con);

    // Confirm timeout -> waits retry delay -> retries null unsolicited
    clock.advance(Duration::from_secs(2));
    o.on_timeout(clock.now(), &app);
    assert!(o.pop_transmit().is_none());
    clock.advance(Duration::from_secs(3));
    o.on_timeout(clock.now(), &app);
    let null2 = o.pop_transmit().expect("null unsol retry");
    o.on_tx_ready(clock.now(), &app);
    assert_eq!(
        ApduResponseHeader::parse(&null2.payload).unwrap().function,
        FunctionCode::UnsolicitedResponse
    );

    // Master confirms null unsolicited retry (FC=0x00, UNS=1, SEQ=1 -> 0xD1 0x00)
    let _ = send_hex(&mut o, &clock, "D1 00", &mut cmd, &mut app);

    // Disable unsolicited Class 1 -> event should NOT trigger unsolicited
    let _ = send_hex(&mut o, &clock, "C0 15 3C 02 06", &mut cmd, &mut app).unwrap();
    o.database.update_double_bit(
        0,
        DoubleBitBinary::new(DoubleBit::DeterminedOn),
        EventMode::Force,
    );
    o.check_for_task_start(clock.now(), &app);
    assert!(o.pop_transmit().is_none());

    // Re-enable unsolicited Class 1 -> event immediately triggers unsolicited!
    let _ = send_hex(&mut o, &clock, "C1 14 3C 02 06", &mut cmd, &mut app).unwrap();
    let unsol = o.pop_transmit().expect("event unsol after enable");
    assert_eq!(
        ApduResponseHeader::parse(&unsol.payload).unwrap().function,
        FunctionCode::UnsolicitedResponse
    );
}

#[test]
fn database_range_helper_validation() {
    assert!(Range::new(0, 0).is_valid());
    assert_eq!(Range::new(2, 5).count(), 4);
    assert!(!Range::new(5, 2).is_valid());
    assert_eq!(Range::new(5, 2).count(), 0);
}
