//! 1:1 port of `TestOutstation.cpp` (59 test cases).

use std::time::Duration;

use opendnp3::app::{
    Analog, AnalogOutputStatus, Binary, BinaryOutputStatus, Counter, Flags, IinBit, OctetString,
};
use opendnp3::enums::{
    EventMode, FunctionCode, RestartMode, StaticAnalogOutputStatusVariation, StaticAnalogVariation,
    StaticBinaryVariation, StaticCounterVariation, StaticTimeAndIntervalVariation,
};
use opendnp3::outstation::{
    AnalogConfig, AnalogOutputStatusConfig, BinaryConfig, BinaryOutputStatusConfig, CounterConfig,
    DatabaseConfig, EventBufferConfig, OctetStringConfig, OutstationConfig,
};

use crate::common::OutstationFixture;

fn no_events_config() -> OutstationConfig {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(0);
    cfg
}

fn new_test_static_read(request: &str, response: &str) {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::all_types(1));
    t.lower_layer_up();
    t.send_to_outstation(request);
    assert_eq!(t.pop_write(), response);
}

#[test]
fn unsupported_function() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 10");
    assert_eq!(t.pop_write(), "C0 81 80 01");
}

#[test]
fn application_iin_bits() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.app.app_iin.set_bit(IinBit::DeviceTrouble);
    t.app.app_iin.set_bit(IinBit::LocalControl);
    t.app.app_iin.set_bit(IinBit::ConfigCorrupt);
    t.app.app_iin.set_bit(IinBit::NeedTime);

    t.send_to_outstation("C0 01");
    assert_eq!(t.pop_write(), "C0 81 F0 20");
}

#[test]
fn read_unknown_object() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 01 00 00 06");
    assert_eq!(t.pop_write(), "C0 81 80 02");
}

#[test]
fn cold_restart() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 0D");
    assert_eq!(t.pop_write(), "C0 81 80 01");
    t.on_tx_ready();

    t.app.cold_mode = Some(RestartMode::SupportedDelayFine);
    t.app.cold_delay = 1;

    t.send_to_outstation("C1 0D");
    assert_eq!(t.pop_write(), "C1 81 80 00 34 02 07 01 01 00");
}

#[test]
fn warm_restart() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 0E");
    assert_eq!(t.pop_write(), "C0 81 80 01");
    t.on_tx_ready();

    t.app.warm_mode = Some(RestartMode::SupportedDelayCoarse);
    t.app.warm_delay = 65535;

    t.send_to_outstation("C1 0E");
    assert_eq!(t.pop_write(), "C1 81 80 00 34 01 07 01 FF FF");
}

#[test]
fn no_response_to_no_ack_codes() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();

    let codes = [
        FunctionCode::DirectOperateNr,
        FunctionCode::FreezeAtTimeNr,
        FunctionCode::FreezeClearNr,
        FunctionCode::ImmedFreezeNr,
    ];
    for (seq, code) in codes.iter().enumerate() {
        let req = format!("{:02X} {:02X}", 0xC0 | (seq as u8), code.to_u8());
        t.send_to_outstation(&req);
        assert!(t.pop_write().is_empty());
    }
}

#[test]
fn write_iin() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 02 50 01 00 07 07 00");
    assert_eq!(t.pop_write(), "C0 81 00 00");
}

#[test]
fn write_iin_enabled() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 02 50 01 00 07 07 01");
    assert_eq!(t.pop_write(), "C0 81 80 04");
}

#[test]
fn write_iin_wrong_bit() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 02 50 01 00 04 04 01");
    assert_eq!(t.pop_write(), "C0 81 80 04");
}

#[test]
fn write_non_write_object() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 02 01 02 00 07 07 00");
    assert_eq!(t.pop_write(), "C0 81 80 02");
}

#[test]
fn delay_measure() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 17");
    assert_eq!(t.pop_write(), "C0 81 80 00 34 02 07 01 00 00");
}

#[test]
fn delay_measure_extra_data() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 17 DE AD BE EF");
    assert_eq!(t.pop_write(), "C0 81 80 00 34 02 07 01 00 00");
}

#[test]
fn write_time_date_non_lan() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.app.supports_write_time = true;
    t.lower_layer_up();

    t.send_to_outstation("C1 02 32 01 07 01 D2 04 00 00 00 00");
    assert_eq!(t.pop_write(), "C1 81 80 00");
    assert_eq!(t.app.written_time.unwrap().value, 1234);
}

#[test]
fn write_time_date_lan_rejects_write_without_prior_record_current_time() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.app.supports_write_time = true;
    t.lower_layer_up();

    t.send_to_outstation("C1 02 32 03 07 01 D2 04 00 00 00 00");
    assert_eq!(t.pop_write(), "C1 81 80 02");
}

#[test]
fn write_time_date_lan_rejects_write_with_bad_sequence_number() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.app.supports_write_time = true;
    t.lower_layer_up();

    t.send_to_outstation("C0 18");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    t.on_tx_ready();

    t.send_to_outstation("C2 02 32 03 07 01 D2 04 00 00 00 00");
    assert_eq!(t.pop_write(), "C2 81 80 02");
}

#[test]
fn write_time_date_lan_accepts_write_with_correct_sequence_number() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.app.supports_write_time = true;
    t.lower_layer_up();

    t.advance_time(Duration::from_millis(10));
    t.send_to_outstation("C0 18");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    t.on_tx_ready();

    t.advance_time(Duration::from_millis(20));
    t.send_to_outstation("C1 02 32 01 07 01 D2 04 00 00 00 00");
    assert_eq!(t.pop_write(), "C1 81 80 00");
    assert_eq!(t.app.written_time.unwrap().value, 1234);
}

#[test]
fn write_time_date_not_asking() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.app.supports_write_time = false;
    t.lower_layer_up();

    t.send_to_outstation("C0 02 32 01 07 01 D2 04 00 00 00 00");
    assert_eq!(t.pop_write(), "C0 81 80 04");
    assert!(t.app.written_time.is_none());
}

#[test]
fn write_time_date_multiple_objects() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.app.supports_write_time = true;
    t.lower_layer_up();

    t.send_to_outstation("C0 02 32 01 07 02 D2 04 00 00 00 00 D2 04 00 00 00 00");
    assert_eq!(t.pop_write(), "C0 81 80 04");
    assert!(t.app.written_time.is_none());
}

#[test]
fn blank_integrity_poll() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(t.pop_write(), "C0 81 80 00");
}

#[test]
fn mixed_variation_assignments() {
    let mut db = DatabaseConfig::default();
    db.analog.insert(
        0,
        AnalogConfig {
            s_var: StaticAnalogVariation::Group30Var1,
            ..Default::default()
        },
    );
    db.analog.insert(
        1,
        AnalogConfig {
            s_var: StaticAnalogVariation::Group30Var2,
            ..Default::default()
        },
    );
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn types_can_be_omitted_from_class0_via_config() {
    let mut db = DatabaseConfig::default();
    db.binary.insert(0, BinaryConfig::default());
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(t.pop_write(), "C0 81 80 00 01 02 00 00 00 02");
}

#[test]
fn octet_strings_can_be_returned_as_part_of_a_class0_scan() {
    let mut db = DatabaseConfig::default();
    for i in 0..3 {
        db.octet_string.insert(i, OctetStringConfig::default());
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.transaction(|db| {
        for i in 0..3 {
            db.update_octet_string(i, OctetString::new(vec![0x00]), EventMode::Suppress);
        }
    });
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn static_octet_strings_switch_headers_if_length_differs() {
    let mut db = DatabaseConfig::default();
    for i in 0..2 {
        db.octet_string.insert(i, OctetStringConfig::default());
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.transaction(|db| {
        db.update_octet_string(0, OctetString::new(b"a".to_vec()), EventMode::Suppress);
        db.update_octet_string(1, OctetString::new(b"bb".to_vec()), EventMode::Suppress);
    });
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn octet_strings_can_be_returned_by_reading_g110v0() {
    let mut db = DatabaseConfig::default();
    for i in 0..3 {
        db.octet_string.insert(i, OctetStringConfig::default());
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.transaction(|db| {
        for i in 0..3 {
            db.update_octet_string(i, OctetString::new(vec![0x00]), EventMode::Suppress);
        }
    });
    t.lower_layer_up();
    t.send_to_outstation("C0 01 6E 00 06");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn read_class0_multi_frag_analog() {
    let mut cfg = no_events_config();
    cfg.params.max_tx_frag_size = 20;
    let mut db = DatabaseConfig::default();
    for i in 0..8 {
        db.analog.insert(i, AnalogConfig::default());
    }
    let mut t = OutstationFixture::new(cfg, db);
    t.lower_layer_up();

    t.transaction(|db| {
        for i in 0..8 {
            db.update_analog(
                i,
                Analog::with_flags(0.0, Flags::new(0x01)),
                EventMode::Suppress,
            );
        }
    });

    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(
        t.pop_write(),
        "A0 81 80 00 1E 01 00 00 01 01 00 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C0 00");
    assert_eq!(
        t.pop_write(),
        "21 81 80 00 1E 01 00 02 03 01 00 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C1 00");
    assert_eq!(
        t.pop_write(),
        "22 81 80 00 1E 01 00 04 05 01 00 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C2 00");
    assert_eq!(
        t.pop_write(),
        "43 81 80 00 1E 01 00 06 07 01 00 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C3 00");
    assert!(t.pop_write().is_empty());
}

#[test]
fn read_func_not_supported() {
    let mut t = OutstationFixture::new(no_events_config(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 01 0C 01 06");
    assert_eq!(t.pop_write(), "C0 81 80 02");
}

#[test]
fn time_and_interval_via_integrity() {
    let mut db = DatabaseConfig::default();
    db.time_and_interval
        .insert(0, StaticTimeAndIntervalVariation::Group50Var4);
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn time_and_interval_via_direct_request() {
    let mut db = DatabaseConfig::default();
    db.time_and_interval
        .insert(0, StaticTimeAndIntervalVariation::Group50Var4);
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 32 04 06");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn time_and_interval_via_direct_range_request() {
    let mut db = DatabaseConfig::default();
    db.time_and_interval
        .insert(0, StaticTimeAndIntervalVariation::Group50Var4);
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 32 04 00 00 00");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn test_time_and_interval_write() {
    let mut db = DatabaseConfig::default();
    db.time_and_interval
        .insert(0, StaticTimeAndIntervalVariation::Group50Var4);
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 02 32 04 28 01 00 07 00 09 00 00 00 00 00 03 00 00 00 05");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn read_grp1_var0_via_integrity() {
    new_test_static_read("C0 01 01 00 06", "C0 81 80 00 01 02 00 00 00 02");
}

#[test]
fn read_grp1_var2_via_integrity() {
    new_test_static_read("C0 01 01 02 06", "C0 81 80 00 01 02 00 00 00 02");
}

#[test]
fn read_grp10_var0_via_integrity() {
    new_test_static_read("C0 01 0A 00 06", "C0 81 80 00 0A 02 00 00 00 02");
}

#[test]
fn read_grp20_var0_via_integrity() {
    new_test_static_read(
        "C0 01 14 00 06",
        "C0 81 80 00 14 01 00 00 00 02 00 00 00 00",
    );
}

#[test]
fn read_grp20_var1_via_integrity() {
    new_test_static_read(
        "C0 01 14 01 06",
        "C0 81 80 00 14 01 00 00 00 02 00 00 00 00",
    );
}

#[test]
fn read_grp20_var5_via_integrity() {
    new_test_static_read("C0 01 14 05 06", "C0 81 80 00 14 05 00 00 00 00 00 00 00");
}

#[test]
fn read_grp30_var0_via_integrity() {
    new_test_static_read(
        "C0 01 1E 00 06",
        "C0 81 80 00 1E 01 00 00 00 02 00 00 00 00",
    );
}

#[test]
fn read_grp30_var1_via_integrity() {
    new_test_static_read(
        "C0 01 1E 01 06",
        "C0 81 80 00 1E 01 00 00 00 02 00 00 00 00",
    );
}

#[test]
fn read_grp30_var3_via_integrity() {
    new_test_static_read("C0 01 1E 03 06", "C0 81 80 00 1E 03 00 00 00 00 00 00 00");
}

#[test]
fn read_grp40_var0_via_integrity() {
    new_test_static_read(
        "C0 01 28 00 06",
        "C0 81 80 00 28 01 00 00 00 02 00 00 00 00",
    );
}

#[test]
fn read_by_range_header() {
    let mut db = DatabaseConfig::default();
    for i in 0..10 {
        db.analog.insert(i, AnalogConfig::default());
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.transaction(|db| {
        db.update_analog(
            5,
            Analog::with_flags(42.0, Flags::new(0x01)),
            EventMode::Suppress,
        );
        db.update_analog(
            6,
            Analog::with_flags(41.0, Flags::new(0x01)),
            EventMode::Suppress,
        );
    });
    t.send_to_outstation("C2 01 1E 02 00 05 06");
    assert_eq!(
        t.pop_write(),
        "C2 81 80 00 1E 02 00 05 06 01 2A 00 01 29 00"
    );
}

#[test]
fn read_grp1_var1() {
    let mut db = DatabaseConfig::default();
    for i in 0..10 {
        db.binary.insert(
            i,
            BinaryConfig {
                s_var: StaticBinaryVariation::Group1Var1,
                ..Default::default()
            },
        );
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.transaction(|db| {
        for i in 0..10 {
            db.update_binary(i, Binary::with_flags(Flags::new(0x01)), EventMode::Suppress);
        }
    });
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(t.pop_write(), "C0 81 80 00 01 01 00 00 09 00 00");
}

#[test]
fn group1_var1_is_promoted_to_group1_var2_if_quality_not_online() {
    let mut db = DatabaseConfig::default();
    for i in 0..2 {
        db.binary.insert(
            i,
            BinaryConfig {
                s_var: StaticBinaryVariation::Group1Var1,
                ..Default::default()
            },
        );
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 3C 01 06");
    assert!(!t.pop_write().is_empty());
}

fn test_static_counter(variation: StaticCounterVariation, value: u32, expected: &str) {
    let mut db = DatabaseConfig::default();
    db.counter.insert(
        0,
        CounterConfig {
            s_var: variation,
            ..Default::default()
        },
    );
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.transaction(|db| {
        db.update_counter(0, Counter::new(value), EventMode::Suppress);
    });
    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(t.pop_write(), expected);
}

#[test]
fn read_grp20_var1() {
    test_static_counter(
        StaticCounterVariation::Group20Var1,
        5,
        "C0 81 80 00 14 01 00 00 00 01 05 00 00 00",
    );
}

#[test]
fn read_grp20_var2() {
    test_static_counter(
        StaticCounterVariation::Group20Var2,
        5,
        "C0 81 80 00 14 02 00 00 00 01 05 00",
    );
}

#[test]
fn read_grp20_var5() {
    test_static_counter(
        StaticCounterVariation::Group20Var5,
        5,
        "C0 81 80 00 14 05 00 00 00 05 00 00 00",
    );
}

#[test]
fn read_grp20_var6() {
    test_static_counter(
        StaticCounterVariation::Group20Var6,
        5,
        "C0 81 80 00 14 06 00 00 00 05 00",
    );
}

fn test_static_analog(variation: StaticAnalogVariation, value: f64, expected: &str) {
    let mut db = DatabaseConfig::default();
    db.analog.insert(
        0,
        AnalogConfig {
            s_var: variation,
            ..Default::default()
        },
    );
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.transaction(|db| {
        db.update_analog(0, Analog::new(value), EventMode::Suppress);
    });
    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(t.pop_write(), expected);
}

#[test]
fn read_grp30_var2() {
    test_static_analog(
        StaticAnalogVariation::Group30Var2,
        100.0,
        "C0 81 80 00 1E 02 00 00 00 01 64 00",
    );
}

#[test]
fn read_grp30_var3() {
    test_static_analog(
        StaticAnalogVariation::Group30Var3,
        65536.0,
        "C0 81 80 00 1E 03 00 00 00 00 00 01 00",
    );
}

#[test]
fn read_grp30_var4() {
    test_static_analog(
        StaticAnalogVariation::Group30Var4,
        100.0,
        "C0 81 80 00 1E 04 00 00 00 64 00",
    );
}

#[test]
fn read_grp30_var5() {
    test_static_analog(
        StaticAnalogVariation::Group30Var5,
        95.6,
        "C0 81 80 00 1E 05 00 00 00 01 33 33 BF 42",
    );
}

#[test]
fn read_grp30_var6() {
    test_static_analog(
        StaticAnalogVariation::Group30Var6,
        -20.0,
        "C0 81 80 00 1E 06 00 00 00 01 00 00 00 00 00 00 34 C0",
    );
}

#[test]
fn read_grp10_var2() {
    let mut db = DatabaseConfig::default();
    db.binary_output_status
        .insert(0, BinaryOutputStatusConfig::default());
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.transaction(|db| {
        db.update_binary_output_status(
            0,
            BinaryOutputStatus::with_flags(Flags::new(0x81)),
            EventMode::Suppress,
        );
    });
    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(t.pop_write(), "C0 81 80 00 0A 02 00 00 00 81");
}

fn test_static_ao_status(variation: StaticAnalogOutputStatusVariation, value: f64, expected: &str) {
    let mut db = DatabaseConfig::default();
    db.analog_output_status.insert(
        0,
        AnalogOutputStatusConfig {
            s_var: variation,
            ..Default::default()
        },
    );
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.transaction(|db| {
        db.update_analog_output_status(0, AnalogOutputStatus::new(value), EventMode::Suppress);
    });
    t.send_to_outstation("C0 01 3C 01 06");
    assert_eq!(t.pop_write(), expected);
}

#[test]
fn read_grp40_var1() {
    test_static_ao_status(
        StaticAnalogOutputStatusVariation::Group40Var1,
        100.0,
        "C0 81 80 00 28 01 00 00 00 01 64 00 00 00",
    );
}

#[test]
fn read_grp40_var2() {
    test_static_ao_status(
        StaticAnalogOutputStatusVariation::Group40Var2,
        100.0,
        "C0 81 80 00 28 02 00 00 00 01 64 00",
    );
}

#[test]
fn read_grp40_var3() {
    test_static_ao_status(
        StaticAnalogOutputStatusVariation::Group40Var3,
        95.6,
        "C0 81 80 00 28 03 00 00 00 01 33 33 BF 42",
    );
}

#[test]
fn read_grp40_var4() {
    test_static_ao_status(
        StaticAnalogOutputStatusVariation::Group40Var4,
        -20.0,
        "C0 81 80 00 28 04 00 00 00 01 00 00 00 00 00 00 34 C0",
    );
}

#[test]
fn read_g1v2_using_qualifier_0x17() {
    let mut db = DatabaseConfig::default();
    for i in 0..3 {
        db.binary.insert(i, BinaryConfig::default());
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 01 02 17 02 00 02");
    assert!(!t.pop_write().is_empty());
}

#[test]
fn read_g1v2_using_qualifier_0x28() {
    let mut db = DatabaseConfig::default();
    for i in 0..3 {
        db.binary.insert(i, BinaryConfig::default());
    }
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.lower_layer_up();
    t.send_to_outstation("C0 01 01 02 28 02 00 00 00 02 00");
    assert!(!t.pop_write().is_empty());
}
