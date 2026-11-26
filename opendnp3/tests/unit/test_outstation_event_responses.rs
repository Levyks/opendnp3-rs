//! 1:1 port of `TestOutstationEventResponses.cpp` (28 test cases).

use std::time::Duration;

use opendnp3::app::{Analog, Binary, Counter, DnpTime, Flags, OctetString};
use opendnp3::enums::{
    EventAnalogVariation, EventClass, EventCounterVariation, EventMode, TimestampQuality,
};
use opendnp3::outstation::{
    BinaryConfig, DatabaseConfig, EventBufferConfig, OctetStringConfig, OutstationConfig,
};

use crate::common::OutstationFixture;

fn test_event_read(
    request: &str,
    expected: &str,
    load: impl FnOnce(&mut opendnp3::outstation::Database),
    configure: impl FnOnce(&mut DatabaseConfig),
) {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(10);
    let mut db = DatabaseConfig::all_types(5);
    configure(&mut db);

    let mut t = OutstationFixture::new(cfg, db);
    t.lower_layer_up();
    t.transaction(load);
    t.send_to_outstation(request);
    assert_eq!(t.pop_write(), expected);
}

#[test]
fn empty_response_when_to_class1_when_no_events_available() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(t.pop_write(), "C0 81 80 00");
}

#[test]
fn receive_new_request_sol_confirm_wait() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(10);
    let mut db = DatabaseConfig::default();
    db.binary.insert(0, BinaryConfig::default());
    let mut t = OutstationFixture::new(cfg, db);
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(t.pop_write(), "E0 81 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();

    t.send_to_outstation("C1 01 3C 02 06");
    assert_eq!(t.pop_write(), "E1 81 80 00 02 01 28 01 00 00 00 81");
}

#[test]
fn read_class1_with_soe() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(10);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::all_types(100));
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_analog(
            0x17,
            Analog::with_flags(0x1234 as f64, Flags::new(0x01)),
            EventMode::Detect,
        );
        db.update_binary(
            0x10,
            Binary::with_flags(Flags::new(0x81)),
            EventMode::Detect,
        );
        db.update_analog(
            0x17,
            Analog::with_flags(0x2222 as f64, Flags::new(0x01)),
            EventMode::Detect,
        );
    });

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(
        t.pop_write(),
        "E0 81 80 00 20 01 28 01 00 17 00 01 34 12 00 00 02 01 28 01 00 10 00 81 20 01 28 01 00 17 00 01 22 22 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C0 00");

    t.send_to_outstation("C1 01 3C 02 06");
    assert_eq!(t.pop_write(), "C1 81 80 00");
}

#[test]
fn read_class1_with_soe_with_timeout_in_between() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(10);
    cfg.params.sol_confirm_timeout = Duration::from_secs(5);
    cfg.params.unsol_confirm_timeout = Duration::from_secs(10);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::all_types(100));
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
    });

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(t.pop_write(), "E0 81 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();

    t.advance_time(Duration::from_secs(5));
    t.send_to_outstation("C0 00");

    t.transaction(|db| {
        db.update_binary(0, Binary::new(false), EventMode::Detect);
    });

    t.send_to_outstation("C1 01 3C 02 06");
    assert_eq!(
        t.pop_write(),
        "E1 81 80 00 02 01 28 02 00 00 00 81 00 00 01"
    );
}

#[test]
fn event_buffer_overflow_and_clear() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(2);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::all_types(100));
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_binary(0, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
        db.update_binary(1, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
        db.update_binary(2, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
    });

    t.send_to_outstation("C0 01");
    assert_eq!(t.pop_write(), "C0 81 82 08");
    t.on_tx_ready();

    t.send_to_outstation("C1 01 3C 02 07 01");
    assert_eq!(t.pop_write(), "E1 81 82 08 02 01 28 01 00 01 00 81");
    t.on_tx_ready();
    t.send_to_outstation("C1 00");

    t.send_to_outstation("C0 01");
    assert_eq!(t.pop_write(), "C0 81 82 00");
}

#[test]
fn multiple_classes() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(10);
    let mut db = DatabaseConfig::all_types(100);
    db.binary.get_mut(&0).unwrap().class = Some(EventClass::Ec1);
    db.analog.get_mut(&0).unwrap().class = Some(EventClass::Ec2);
    db.counter.get_mut(&0).unwrap().class = Some(EventClass::Ec3);

    let mut t = OutstationFixture::new(cfg, db);
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
        db.update_analog(0, Analog::new(3.0), EventMode::Detect);
        db.update_counter(0, Counter::new(7), EventMode::Detect);
    });

    t.send_to_outstation("C0 01");
    assert_eq!(t.pop_write(), "C0 81 8E 00");
    t.on_tx_ready();

    t.send_to_outstation("C1 01 3C 03 06");
    assert_eq!(
        t.pop_write(),
        "E1 81 8A 00 20 01 28 01 00 00 00 01 03 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C1 00");

    t.send_to_outstation("C2 01 3C 04 06");
    assert_eq!(
        t.pop_write(),
        "E2 81 82 00 16 01 28 01 00 00 00 01 07 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C2 00");

    t.send_to_outstation("C3 01 3C 02 06");
    assert_eq!(t.pop_write(), "E3 81 80 00 02 01 28 01 00 00 00 81");
    t.on_tx_ready();
    t.send_to_outstation("C3 00");

    t.send_to_outstation("C4 01");
    assert_eq!(t.pop_write(), "C4 81 80 00");
}

#[test]
fn class1() {
    test_event_read(
        "C0 01 3C 02 06",
        "E0 81 80 00 02 01 28 01 00 00 00 01",
        |db| {
            db.update_binary(0, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
        },
        |_| {},
    );
}

#[test]
fn class1_one_byte_limited_count() {
    test_event_read(
        "C0 01 3C 02 07 01",
        "E0 81 82 00 02 01 28 01 00 02 00 01",
        |db| {
            db.update_binary(2, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
            db.update_binary(1, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
        },
        |_| {},
    );
}

#[test]
fn class1_two_byte_limited_count() {
    test_event_read(
        "C0 01 3C 02 08 02 00",
        "E0 81 82 00 02 01 28 02 00 02 00 01 01 00 81",
        |db| {
            db.update_binary(2, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
            db.update_binary(1, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
            db.update_binary(3, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
        },
        |_| {},
    );
}

#[test]
fn mixed_class_limited_count() {
    test_event_read(
        "C0 01 3C 03 07 01 3C 04 07 01",
        "E0 81 8E 00 02 01 28 02 00 01 00 81 02 00 81",
        |db| {
            db.update_binary(0, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
            db.update_binary(1, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
            db.update_binary(1, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
            db.update_binary(2, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
            db.update_binary(2, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
        },
        |db| {
            db.binary.get_mut(&0).unwrap().class = Some(EventClass::Ec1);
            db.binary.get_mut(&1).unwrap().class = Some(EventClass::Ec2);
            db.binary.get_mut(&2).unwrap().class = Some(EventClass::Ec3);
        },
    );
}

#[test]
fn reports_g22v5_correctly() {
    test_event_read(
        "C0 01 3C 02 06",
        "E0 81 80 00 16 05 28 01 00 00 00 01 17 00 00 00 78 E6 B7 2D 60 01",
        |db| {
            db.update_counter(
                0,
                Counter::with_flags_and_time(23, Flags::new(0x01), DnpTime::new(1512595515000)),
                EventMode::Detect,
            );
        },
        |db| {
            db.counter.get_mut(&0).unwrap().e_var = EventCounterVariation::Group22Var5;
        },
    );
}

#[test]
fn reports_g22v6_correctly() {
    test_event_read(
        "C0 01 3C 02 06",
        "E0 81 80 00 16 06 28 01 00 00 00 01 17 00 78 E6 B7 2D 60 01",
        |db| {
            db.update_counter(
                0,
                Counter::with_flags_and_time(23, Flags::new(0x01), DnpTime::new(1512595515000)),
                EventMode::Detect,
            );
        },
        |db| {
            db.counter.get_mut(&0).unwrap().e_var = EventCounterVariation::Group22Var6;
        },
    );
}

#[test]
fn read_grp2_var0() {
    test_event_read(
        "C0 01 02 00 06",
        "E0 81 80 00 02 01 28 01 00 00 00 01",
        |db| {
            db.update_binary(0, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
        },
        |_| {},
    );
}

#[test]
fn read_grp22_var0() {
    test_event_read(
        "C0 01 16 00 06",
        "E0 81 80 00 16 01 28 01 00 00 00 01 00 00 00 00",
        |db| {
            db.update_counter(
                0,
                Counter::with_flags(0, Flags::new(0x01)),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp32_var0() {
    test_event_read(
        "C0 01 20 00 06",
        "E0 81 80 00 20 01 28 01 00 00 00 01 00 00 00 00",
        |db| {
            db.update_analog(
                0,
                Analog::with_flags(0.0, Flags::new(0x01)),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp32_var7() {
    let response = "E0 81 80 00 20 07 28 01 00 00 00 01 00 00 00 00 06 05 04 03 02 01";
    let update = |db: &mut opendnp3::outstation::Database| {
        db.update_analog(
            0,
            Analog::with_flags_and_time(0.0, Flags::new(0x01), DnpTime::new(0x010203040506)),
            EventMode::Detect,
        );
    };
    test_event_read("C0 01 20 07 06", response, update, |db| {
        db.analog.get_mut(&0).unwrap().e_var = EventAnalogVariation::Group32Var1;
    });
    test_event_read("C0 01 20 00 06", response, update, |db| {
        db.analog.get_mut(&0).unwrap().e_var = EventAnalogVariation::Group32Var7;
    });
}

#[test]
fn read_grp32_var5() {
    let response = "E0 81 80 00 20 05 28 01 00 00 00 01 00 00 00 00";
    let update = |db: &mut opendnp3::outstation::Database| {
        db.update_analog(
            0,
            Analog::with_flags_and_time(0.0, Flags::new(0x01), DnpTime::new(0x010203040506)),
            EventMode::Detect,
        );
    };
    test_event_read("C0 01 20 05 06", response, update, |db| {
        db.analog.get_mut(&0).unwrap().e_var = EventAnalogVariation::Group32Var1;
    });
    test_event_read("C0 01 20 00 06", response, update, |db| {
        db.analog.get_mut(&0).unwrap().e_var = EventAnalogVariation::Group32Var5;
    });
}

#[test]
fn read_grp2_var1() {
    test_event_read(
        "C0 01 02 01 06",
        "E0 81 80 00 02 01 28 01 00 03 00 01",
        |db| {
            db.update_binary(3, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var1_limited_count() {
    test_event_read(
        "C0 01 02 01 07 02",
        "E0 81 82 00 02 01 28 02 00 03 00 01 02 00 81",
        |db| {
            db.update_binary(3, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
            db.update_binary(2, Binary::with_flags(Flags::new(0x81)), EventMode::Detect);
            db.update_binary(1, Binary::with_flags(Flags::new(0x01)), EventMode::Detect);
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var2() {
    test_event_read(
        "C0 01 02 02 06",
        "E0 81 80 00 02 02 28 01 00 03 00 01 71 45 00 00 00 00",
        |db| {
            db.update_binary(
                3,
                Binary::with_flags_and_time(Flags::new(0x01), DnpTime::new(0x4571)),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var3_single_value() {
    test_event_read(
        "C0 01 02 03 06",
        "E0 81 80 00 33 02 07 01 71 45 00 00 00 00 02 03 28 01 00 03 00 01 00 00",
        |db| {
            db.update_binary(
                3,
                Binary::with_flags_and_time(
                    Flags::new(0x01),
                    DnpTime::with_quality(0x4571, TimestampQuality::Unsynchronized),
                ),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var3_two_values() {
    test_event_read(
        "C0 01 02 03 06",
        "E0 81 80 00 33 01 07 01 71 45 00 00 00 00 02 03 28 02 00 03 00 01 00 00 04 00 81 08 00",
        |db| {
            db.update_binary(
                3,
                Binary::with_flags_and_time(
                    Flags::new(0x01),
                    DnpTime::with_quality(0x4571, TimestampQuality::Synchronized),
                ),
                EventMode::Detect,
            );
            db.update_binary(
                4,
                Binary::with_flags_and_time(
                    Flags::new(0x81),
                    DnpTime::with_quality(0x4579, TimestampQuality::Synchronized),
                ),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var3_two_values_negative_difference() {
    test_event_read(
        "C0 01 02 03 06",
        "E0 81 80 00 33 01 07 01 71 45 00 00 00 00 02 03 28 01 00 03 00 01 00 00 33 01 07 01 70 45 00 00 00 00 02 03 28 01 00 04 00 81 00 00",
        |db| {
            db.update_binary(
                3,
                Binary::with_flags_and_time(
                    Flags::new(0x01),
                    DnpTime::with_quality(0x4571, TimestampQuality::Synchronized),
                ),
                EventMode::Detect,
            );
            db.update_binary(
                4,
                Binary::with_flags_and_time(
                    Flags::new(0x81),
                    DnpTime::with_quality(0x4570, TimestampQuality::Synchronized),
                ),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var3_two_values_difference_too_big_for_cto() {
    test_event_read(
        "C0 01 02 03 06",
        "E0 81 80 00 33 01 07 01 00 00 00 00 00 00 02 03 28 01 00 03 00 01 00 00 33 01 07 01 00 00 01 00 00 00 02 03 28 01 00 04 00 81 00 00",
        |db| {
            db.update_binary(
                3,
                Binary::with_flags_and_time(
                    Flags::new(0x01),
                    DnpTime::with_quality(0x000000, TimestampQuality::Synchronized),
                ),
                EventMode::Detect,
            );
            db.update_binary(
                4,
                Binary::with_flags_and_time(
                    Flags::new(0x81),
                    DnpTime::with_quality(0x010000, TimestampQuality::Synchronized),
                ),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var3_two_values_with_different_time_quality() {
    test_event_read(
        "C0 01 02 03 06",
        "E0 81 80 00 33 01 07 01 71 45 00 00 00 00 02 03 28 01 00 03 00 01 00 00 33 02 07 01 79 45 00 00 00 00 02 03 28 01 00 04 00 81 00 00",
        |db| {
            db.update_binary(
                3,
                Binary::with_flags_and_time(
                    Flags::new(0x01),
                    DnpTime::with_quality(0x4571, TimestampQuality::Synchronized),
                ),
                EventMode::Detect,
            );
            db.update_binary(
                4,
                Binary::with_flags_and_time(
                    Flags::new(0x81),
                    DnpTime::with_quality(0x4579, TimestampQuality::Unsynchronized),
                ),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn read_grp2_var3_two_values_with_different_invalid_or_unsynchronized_are_reported_together() {
    test_event_read(
        "C0 01 02 03 06",
        "E0 81 80 00 33 02 07 01 71 45 00 00 00 00 02 03 28 02 00 03 00 01 00 00 04 00 81 08 00",
        |db| {
            db.update_binary(
                3,
                Binary::with_flags_and_time(
                    Flags::new(0x01),
                    DnpTime::with_quality(0x4571, TimestampQuality::Unsynchronized),
                ),
                EventMode::Detect,
            );
            db.update_binary(
                4,
                Binary::with_flags_and_time(
                    Flags::new(0x81),
                    DnpTime::with_quality(0x4579, TimestampQuality::Invalid),
                ),
                EventMode::Detect,
            );
        },
        |_| {},
    );
}

#[test]
fn reports_octet_string_events_with_same_size_in_same_header() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut db = DatabaseConfig::default();
    for i in 0..5 {
        db.octet_string.insert(i, OctetStringConfig::default());
    }
    let mut t = OutstationFixture::new(cfg, db);
    t.lower_layer_up();

    t.transaction(|db| {
        let data = OctetString::new(vec![0xCA, 0xFE]);
        db.update_octet_string(2, data.clone(), EventMode::Detect);
        db.update_octet_string(1, data, EventMode::Detect);
    });

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(
        t.pop_write(),
        "E0 81 80 00 6F 02 28 02 00 02 00 CA FE 01 00 CA FE"
    );
}

#[test]
fn reports_octet_string_events_with_different_sizes_in_separate_headers() {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(5);
    let mut db = DatabaseConfig::default();
    for i in 0..5 {
        db.octet_string.insert(i, OctetStringConfig::default());
    }
    let mut t = OutstationFixture::new(cfg, db);
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_octet_string(0, OctetString::new(vec![0xCA, 0xFE]), EventMode::Detect);
        db.update_octet_string(
            0,
            OctetString::new(vec![0xCA, 0xFE, 0xBA, 0xBE]),
            EventMode::Detect,
        );
    });

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(
        t.pop_write(),
        "E0 81 80 00 6F 02 28 01 00 00 00 CA FE 6F 04 28 01 00 00 00 CA FE BA BE"
    );
}
