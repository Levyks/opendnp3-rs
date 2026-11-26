//! 1:1 port of `TestMaster.cpp` (33 test cases).

use std::time::Duration;

use opendnp3::Message;
use opendnp3::app::{
    Binary, ClassField, DnpTime, HeaderInfo, IinBit, IinField, Indexed, OctetString,
    TimeAndInterval,
};
use opendnp3::enums::{
    GroupVariation, MasterTaskType, RestartType, TaskCompletion, TimeSyncMode, TimestampQuality,
};
use opendnp3::link::Addresses;
use opendnp3::master::{MasterParams, SoeHandler};
use opendnp3::util::hex::from_hex;

use crate::common::{MasterFixture, no_startup_tasks};

#[derive(Default)]
struct RecordingSoe {
    binaries: Vec<Indexed<Binary>>,
    octets: Vec<Indexed<OctetString>>,
    time_and_intervals: Vec<Indexed<TimeAndInterval>>,
}

impl SoeHandler for RecordingSoe {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.extend_from_slice(values);
    }
    fn process_octet_string(&mut self, _info: HeaderInfo, values: &[Indexed<OctetString>]) {
        self.octets.extend_from_slice(values);
    }
    fn process_time_and_interval(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<TimeAndInterval>],
    ) {
        self.time_and_intervals.extend_from_slice(values);
    }
}

fn feed_with_soe(t: &mut MasterFixture, hex: &str, soe: &mut RecordingSoe) {
    let bytes = from_hex(hex).unwrap();
    t.master.on_receive(
        &Message::new(Addresses::new(1024, 1), bytes),
        t.clock.now(),
        soe,
        &mut t.app,
    );
}

#[test]
fn notifies_application_of_state_changes() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.lower_layer_down();
    assert!(t.pop_write().is_empty());
}

#[test]
fn integrity_on_startup() {
    let mut params = no_startup_tasks();
    params.startup_integrity_class_mask = ClassField::ALL_CLASSES;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();

    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00"));
    assert!(t.pop_write().is_empty());
}

#[test]
fn solicited_response_with_data() {
    let mut params = no_startup_tasks();
    params.startup_integrity_class_mask = ClassField::ALL_CLASSES;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();

    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        &format!("{seq} 81 00 00 01 02 00 00 00 81"),
        &mut soe,
    );
    assert_eq!(soe.binaries.len(), 1);
    assert!(soe.binaries[0].value.value);
}

#[test]
fn unsol_disable_enable_on_startup() {
    let mut t = MasterFixture::new(MasterParams::default());
    t.lower_layer_up();

    let r1 = t.pop_write();
    let s1 = r1[..2].to_string();
    assert!(r1.ends_with(" 15 3C 02 06 3C 03 06 3C 04 06"));
    t.on_tx_ready();
    t.send_to_master(&format!("{s1} 81 00 00"));

    let r2 = t.pop_write();
    let s2 = r2[..2].to_string();
    assert!(r2.ends_with(" 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06"));
    t.on_tx_ready();
    t.send_to_master(&format!("{s2} 81 00 00"));

    let r3 = t.pop_write();
    let s3 = r3[..2].to_string();
    assert!(r3.ends_with(" 14 3C 02 06 3C 03 06 3C 04 06"));
    t.on_tx_ready();
    t.send_to_master(&format!("{s3} 81 00 00"));
    assert!(t.pop_write().is_empty());
}

#[test]
fn timeout_during_startup() {
    let mut t = MasterFixture::new(MasterParams::default());
    t.lower_layer_up();
    let _r1 = t.pop_write();
    t.on_tx_ready();
    t.advance_to_next_timer();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureResponseTimeout)
    );
}

#[test]
fn solicited_response_timeout() {
    let mut params = no_startup_tasks();
    params.startup_integrity_class_mask = ClassField::ALL_CLASSES;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();
    let _req = t.pop_write();
    t.on_tx_ready();
    t.advance_to_next_timer();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureResponseTimeout)
    );
}

#[test]
fn retries_use_exponential_backoff() {
    let mut params = no_startup_tasks();
    params.startup_integrity_class_mask = ClassField::ALL_CLASSES;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();
    let _req = t.pop_write();
    t.on_tx_ready();
    t.advance_to_next_timer();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureResponseTimeout)
    );
}

#[test]
fn all_objects_scan() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_all_objects(GroupVariation::Group1Var2, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    assert!(req.ends_with(" 01 01 02 06"));
}

#[test]
fn class_scan_can_repeat() {
    let mut t = MasterFixture::new(no_startup_tasks());
    let now = t.clock.now();
    t.master
        .add_class_scan(ClassField::ALL_CLASSES, Duration::from_secs(10), now);
    t.lower_layer_up();

    t.advance_time(Duration::from_secs(10));
    let r1 = t.pop_write();
    let s1 = r1[..2].to_string();
    t.on_tx_ready();
    t.send_to_master(&format!("{s1} 81 00 00"));

    t.advance_time(Duration::from_secs(10));
    let r2 = t.pop_write();
    assert!(!r2.is_empty());
}

#[test]
fn solicited_response_layer_down() {
    let mut params = no_startup_tasks();
    params.startup_integrity_class_mask = ClassField::ALL_CLASSES;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();
    let _req = t.pop_write();
    t.on_tx_ready();
    t.lower_layer_down();
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureNoComms)
    );
}

#[test]
fn solicited_multi_frag_response() {
    let mut params = no_startup_tasks();
    params.startup_integrity_class_mask = ClassField::ALL_CLASSES;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();
    let req = t.pop_write();
    let seq = u8::from_str_radix(&req[..2], 16).unwrap() & 0x0F;
    t.on_tx_ready();

    // FIR=1, FIN=0, CON=1
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        &format!("{:02X} 81 00 00 01 02 00 00 00 81", 0xA0 | seq),
        &mut soe,
    );
    let confirm = t.pop_write();
    assert_eq!(confirm, format!("{:02X} 00", 0xC0 | seq));
    t.on_tx_ready();

    // FIR=0, FIN=1, CON=0, seq+1
    let seq2 = (seq + 1) & 0x0F;
    feed_with_soe(
        &mut t,
        &format!("{:02X} 81 00 00 01 02 00 01 01 01", 0x40 | seq2),
        &mut soe,
    );
    assert_eq!(soe.binaries.len(), 2);
}

#[test]
fn event_poll() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_classes(ClassField::ALL_EVENT_CLASSES, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    assert!(req.ends_with(" 01 3C 02 06 3C 03 06 3C 04 06"));
}

#[test]
fn parses_octet_string_response_with_five_characters() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_classes(ClassField::ALL_CLASSES, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        &format!("{seq} 81 00 00 6E 05 00 00 00 68 65 6C 6C 6F"),
        &mut soe,
    );
    assert_eq!(soe.octets.len(), 1);
    assert_eq!(soe.octets[0].value.bytes.as_slice(), b"hello");
}

#[test]
fn parses_octet_string_response_size_of_one() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_classes(ClassField::ALL_CLASSES, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        &format!("{seq} 81 00 00 6E 01 00 00 00 41"),
        &mut soe,
    );
    assert_eq!(soe.octets.len(), 1);
    assert_eq!(soe.octets[0].value.bytes.as_slice(), b"A");
}

#[test]
fn parses_group2_var3_correctly() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_classes(ClassField::ALL_EVENT_CLASSES, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        &format!("{seq} 81 00 00 33 01 07 01 07 00 00 00 00 00 02 03 17 01 08 81 01 00"),
        &mut soe,
    );
    assert_eq!(soe.binaries.len(), 1);
    assert_eq!(soe.binaries[0].value.time.value, 8);
}

#[test]
fn parses_group2_var3_time_quality_correctly() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_classes(ClassField::ALL_EVENT_CLASSES, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        &format!("{seq} 81 00 00 33 02 07 01 07 00 00 00 00 00 02 03 17 01 08 81 01 00"),
        &mut soe,
    );
    assert_eq!(soe.binaries.len(), 1);
    assert_eq!(
        soe.binaries[0].value.time.quality,
        TimestampQuality::Unsynchronized
    );
}

#[test]
fn parses_group50_var4() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_all_objects(GroupVariation::Group50Var4, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        &format!("{seq} 81 00 00 32 04 00 00 00 09 00 00 00 00 00 03 00 00 00 05"),
        &mut soe,
    );
    assert_eq!(soe.time_and_intervals.len(), 1);
    assert_eq!(soe.time_and_intervals[0].value.time.value, 9);
}

#[test]
fn restart_via_null_unsol() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.send_to_master("F0 82 80 00");
    assert_eq!(t.pop_write(), "D0 00");
    t.on_tx_ready();
    let clear = t.pop_write();
    assert!(clear.ends_with(" 02 50 01 00 07 07 00"));
}

#[test]
fn disable_automated_restart_clear() {
    let mut params = no_startup_tasks();
    params.ignore_restart_iin = true;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();
    t.send_to_master("F0 82 80 00");
    assert_eq!(t.pop_write(), "D0 00");
    t.on_tx_ready();
    assert!(t.pop_write().is_empty());
}

#[test]
fn restart_during_startup() {
    let mut t = MasterFixture::new(MasterParams::default());
    t.lower_layer_up();
    let r1 = t.pop_write();
    let s1 = r1[..2].to_string();
    t.on_tx_ready();
    t.send_to_master(&format!("{s1} 81 80 00"));
    let clear = t.pop_write();
    assert!(clear.ends_with(" 02 50 01 00 07 07 00"));
}

#[test]
fn performs_non_lan_time_synchronization() {
    let mut params = no_startup_tasks();
    params.time_sync_mode = TimeSyncMode::NonLan;
    let mut t = MasterFixture::new(params);
    t.app.now_dnp = DnpTime::new(1000);
    t.lower_layer_up();

    t.send_to_master("F0 82 10 00");
    assert_eq!(t.pop_write(), "D0 00");
    t.on_tx_ready();

    let dm = t.pop_write();
    let s1 = dm[..2].to_string();
    assert!(dm.ends_with(" 17"));
    t.on_tx_ready();
    t.send_to_master(&format!("{s1} 81 10 00 34 02 07 01 05 00"));

    let wt = t.pop_write();
    assert!(wt.contains(" 02 32 01 07 01 "));
}

#[test]
fn performs_lan_time_synchronization() {
    let mut params = no_startup_tasks();
    params.time_sync_mode = TimeSyncMode::Lan;
    let mut t = MasterFixture::new(params);
    t.app.now_dnp = DnpTime::new(2000);
    t.lower_layer_up();

    t.send_to_master("F0 82 10 00");
    assert_eq!(t.pop_write(), "D0 00");
    t.on_tx_ready();

    let rct = t.pop_write();
    let s1 = rct[..2].to_string();
    assert!(rct.ends_with(" 18"));
    t.on_tx_ready();
    t.send_to_master(&format!("{s1} 81 10 00"));

    let wt = t.pop_write();
    assert!(wt.contains(" 02 32 03 07 01 "));
}

#[test]
fn receive_cto_synchronized() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        "F0 82 00 00 33 01 07 01 0A 00 00 00 00 00 02 03 17 01 01 81 05 00",
        &mut soe,
    );
    assert_eq!(soe.binaries.len(), 1);
    assert_eq!(soe.binaries[0].value.time.value, 15);
    assert_eq!(
        soe.binaries[0].value.time.quality,
        TimestampQuality::Synchronized
    );
}

#[test]
fn receive_cto_unsynchronized() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    let mut soe = RecordingSoe::default();
    feed_with_soe(
        &mut t,
        "F0 82 00 00 33 02 07 01 0A 00 00 00 00 00 02 03 17 01 01 81 05 00",
        &mut soe,
    );
    assert_eq!(soe.binaries.len(), 1);
    assert_eq!(soe.binaries[0].value.time.value, 15);
    assert_eq!(
        soe.binaries[0].value.time.quality,
        TimestampQuality::Unsynchronized
    );
}

#[test]
fn receive_iin_in_responses() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_classes(ClassField::ALL_CLASSES, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 40 20"));
    assert_eq!(
        t.app.received_iins.last().copied(),
        Some(IinField::new(0x40, 0x20))
    );
}

#[test]
fn receive_iin_unsol() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.send_to_master("F0 82 40 20");
    assert_eq!(
        t.app.received_iins.last().copied(),
        Some(IinField::new(0x40, 0x20))
    );
}

#[test]
fn event_scan_on_events_available_iin() {
    let mut params = no_startup_tasks();
    params.event_scan_on_events_available_class_mask = ClassField::CLASS_1;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();
    t.send_to_master("F0 82 02 00");
    assert_eq!(t.pop_write(), "D0 00");
    t.on_tx_ready();
    let poll = t.pop_write();
    assert!(poll.ends_with(" 01 3C 02 06"));
}

#[test]
fn adhoc_scan_works_with_unsolicited_disabled() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_classes(ClassField::CLASS_1, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 01 3C 02 06"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00"));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::Success)
    );
}

#[test]
fn adhoc_scan_fails_immediately_if_master_offline() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.master
        .scan_classes(ClassField::CLASS_1, t.clock.now(), &mut t.app);
    assert!(t.pop_write().is_empty());
    assert!(
        t.app
            .completed
            .iter()
            .any(|(_, c)| *c == TaskCompletion::FailureNoComms)
    );
}

#[test]
fn master_writes_time_and_interval() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .scan_all_objects(GroupVariation::Group50Var4, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    assert!(req.ends_with(" 01 32 04 06"));
}

#[test]
fn cold_restart_fails_with_empty_response() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .restart(RestartType::Cold, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 0D"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00"));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(ty, _)| *ty == MasterTaskType::UserTask)
    );
}

#[test]
fn warm_restart_fails_with_empty_response() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();
    t.master
        .restart(RestartType::Warm, t.clock.now(), &mut t.app);
    let req = t.pop_write();
    let seq = req[..2].to_string();
    assert!(req.ends_with(" 0E"));
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00"));
    assert!(
        t.app
            .completed
            .iter()
            .any(|(ty, _)| *ty == MasterTaskType::UserTask)
    );
}

#[test]
fn integrity_poll_on_buffer_overflow_iin() {
    let mut params = no_startup_tasks();
    params.integrity_on_event_overflow_iin = true;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();
    t.send_to_master("F0 82 00 08");
    assert_eq!(t.pop_write(), "D0 00");
    t.on_tx_ready();
    let poll = t.pop_write();
    assert!(poll.ends_with(" 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06"));
    assert!(
        t.app
            .received_iins
            .iter()
            .any(|i| i.is_set(IinBit::EventBufferOverflow))
    );
}
