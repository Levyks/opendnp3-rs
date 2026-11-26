//! 1:1 port of `TestOutstationUnsolicitedResponses.cpp` (19 test cases).

use std::time::Duration;

use opendnp3::app::{Analog, Binary, ClassField, Flags};
use opendnp3::enums::{EventClass, EventMode};
use opendnp3::outstation::{
    AnalogConfig, BinaryConfig, DatabaseConfig, EventBufferConfig, OutstationConfig,
};

use crate::common::OutstationFixture;

fn binary_db(count: u16) -> DatabaseConfig {
    let mut db = DatabaseConfig::default();
    for i in 0..count {
        db.binary.insert(i, BinaryConfig::default());
    }
    db
}

#[test]
fn sends_null_unsol_on_startup() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    assert!(t.os.next_wakeup().is_some());
}

#[test]
fn non_read_during_null_unsol_without_workaround() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    assert!(t.has_no_data());

    t.send_to_outstation("C0 02 50 01 00 07 07 00");
    assert_eq!(t.pop_write(), "C0 81 00 00");
    t.on_tx_ready();

    t.advance_time(Duration::from_secs(5));
    t.advance_time(Duration::from_secs(5));
    assert_eq!(t.pop_write(), "F1 82 00 00");
}

#[test]
fn non_read_during_null_unsol_with_workaround() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();

    t.send_to_outstation("C0 02 50 01 00 07 07 00");
    assert_eq!(t.pop_write(), "C0 81 00 00");
    t.on_tx_ready();

    t.advance_time(Duration::from_secs(5));
    t.advance_time(Duration::from_secs(5));
    assert_eq!(t.pop_write(), "F1 82 00 00");
}

#[test]
fn read_during_null_unsol_without_workaround() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    t.on_tx_ready();
}

#[test]
fn read_during_null_unsol_with_workaround() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    t.on_tx_ready();
}

#[test]
fn unsol_confirm_timeout() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    cfg.params.unsol_retry_delay = Duration::ZERO;
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();

    assert!(t.os.next_wakeup().is_some());
    t.advance_time(Duration::from_secs(5));
    assert_eq!(t.pop_write(), "F1 82 80 00");
}

#[test]
fn unsol_num_retries() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    cfg.params.unsol_retry_delay = Duration::ZERO;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(1));
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
    });
    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();

    t.advance_time(Duration::from_secs(5));
    assert!(!t.pop_write().is_empty());
}

#[test]
fn unsol_infinite_retries() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    cfg.params.unsol_retry_delay = Duration::ZERO;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(1));
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
    });
    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();

    for _ in 0..10 {
        t.advance_time(Duration::from_secs(5));
        assert!(!t.pop_write().is_empty());
        t.on_tx_ready();
    }
}

#[test]
fn unsol_data() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);

    let mut db = binary_db(3);
    db.binary.get_mut(&0).unwrap().class = Some(EventClass::Ec1);
    db.binary.get_mut(&1).unwrap().class = Some(EventClass::Ec2);
    db.binary.get_mut(&2).unwrap().class = Some(EventClass::Ec3);

    let mut t = OutstationFixture::new(cfg, db);
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(2, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
    });

    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 02 00 01");
    t.on_tx_ready();
    t.send_to_outstation("D1 00");
    assert!(t.pop_write().is_empty());
}

#[test]
fn unsol_event_buffer_overflow() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::CLASS_1;
    cfg.event_buffer = EventBufferConfig::all_types(2);
    let mut t = OutstationFixture::new(cfg, binary_db(1));

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
        db.update_binary(0, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });

    assert_eq!(
        t.pop_write(),
        "F1 82 80 08 02 01 28 02 00 00 00 01 00 00 81"
    );
    t.on_tx_ready();
    t.send_to_outstation("D1 00");
    assert!(t.pop_write().is_empty());
}

#[test]
fn unsol_multi_fragments() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.max_tx_frag_size = 20;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut db = DatabaseConfig::default();
    for i in 0..5 {
        db.analog.insert(i, AnalogConfig::default());
    }
    let mut t = OutstationFixture::new(cfg, db);

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");
    assert!(t.pop_write().is_empty());

    t.transaction(|db| {
        db.update_analog(
            1,
            Analog::with_flags(7.0, Flags::new(0x01)),
            EventMode::Detect,
        );
        db.update_analog(
            3,
            Analog::with_flags(13.0, Flags::new(0x01)),
            EventMode::Detect,
        );
    });

    assert_eq!(
        t.pop_write(),
        "F1 82 82 00 20 01 28 01 00 01 00 01 07 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("D1 00");

    assert_eq!(
        t.pop_write(),
        "F2 82 80 00 20 01 28 01 00 03 00 01 0D 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("D2 00");
    assert!(t.pop_write().is_empty());
}

fn write_during_unsol(before_tx: bool) {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(5));

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });
    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 00 00 81");

    if before_tx {
        t.send_to_outstation("C0 02 50 01 00 07 07 00");
        t.on_tx_ready();
    } else {
        t.on_tx_ready();
        t.send_to_outstation("C0 02 50 01 00 07 07 00");
    }

    assert_eq!(t.pop_write(), "C0 81 00 00");

    t.send_to_outstation("D1 00");
    assert!(t.pop_write().is_empty());
}

#[test]
fn write_during_unsol_before_tx() {
    write_during_unsol(true);
}

#[test]
fn write_during_unsol_after_tx() {
    write_during_unsol(false);
}

#[test]
fn read_during_unsol_confirm() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(5));

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });
    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();

    t.send_to_outstation("D1 00");
    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(t.pop_write(), "C0 81 80 00");
}

#[test]
fn read_during_unsol_without_confirm() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    cfg.params.unsol_confirm_timeout = Duration::from_secs(5);
    let mut t = OutstationFixture::new(cfg, binary_db(5));

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });
    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();

    t.advance_time(Duration::from_secs(5));
    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(t.pop_write(), "E0 81 80 00 02 01 28 01 00 00 00 81");
}

#[test]
fn read_write_during_unsol() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(5));

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });
    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();

    t.send_to_outstation("C0 02 50 01 00 07 07 00");
    assert_eq!(t.pop_write(), "C0 81 00 00");
}

#[test]
fn repeat_request_during_unsol() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::ALL_EVENT_CLASSES;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(5));

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.send_to_outstation("C0 02 50 01 00 07 07 00");
    assert_eq!(t.pop_write(), "C0 81 00 00");
    t.on_tx_ready();

    t.transaction(|db| {
        db.update_binary(1, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });
    assert_eq!(t.pop_write(), "F1 82 00 00 02 01 28 01 00 01 00 81");
    t.on_tx_ready();

    t.send_to_outstation("C0 02 50 01 00 07 07 00");
    assert_eq!(t.pop_write(), "C0 81 00 00");
}

#[test]
fn unsol_enable() {
    let mut cfg = OutstationConfig::default();
    cfg.params.allow_unsolicited = true;
    cfg.params.unsol_class_mask = ClassField::NONE;
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(5));

    t.lower_layer_up();
    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();
    t.send_to_outstation("D0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
    });
    assert!(t.pop_write().is_empty());

    t.send_to_outstation("C0 14 3C 02 06");
    assert_eq!(t.pop_write(), "C0 81 82 00");
    t.on_tx_ready();

    assert_eq!(t.pop_write(), "F1 82 80 00 02 01 28 01 00 00 00 01");
}

#[test]
fn unsol_enable_disable_failure() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut t = OutstationFixture::new(cfg, binary_db(5));

    t.lower_layer_up();
    t.send_to_outstation("C0 14 3C 02 06");
    assert_eq!(t.pop_write(), "C0 81 80 01");
}
