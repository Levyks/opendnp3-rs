//! 1:1 port of `TestOutstationFrozenCounters.cpp` (6 test cases).

use opendnp3::app::{Counter, DnpTime, Flags, FrozenCounter};
use opendnp3::enums::{EventMode, StaticFrozenCounterVariation, TimestampQuality};
use opendnp3::link::LinkBroadcastAddress;
use opendnp3::outstation::{
    CounterConfig, DatabaseConfig, EventBufferConfig, FrozenCounterConfig, OutstationConfig,
};

use crate::common::OutstationFixture;

const COUNTER_VALUE: u32 = 41;
const COUNTER_VALUE_HEX: &str = "29";
const FREEZE_TIMESTAMP_HEX: &str = "C0 AA 57 05 38 00";

fn freeze_timestamp() -> DnpTime {
    DnpTime::with_quality(240_607_800_000, TimestampQuality::Synchronized)
}

fn counter_db(count: u16, frozen_var: StaticFrozenCounterVariation) -> DatabaseConfig {
    let mut db = DatabaseConfig::default();
    for i in 0..count {
        db.counter.insert(i, CounterConfig::default());
        db.frozen_counter.insert(
            i,
            FrozenCounterConfig {
                s_var: frozen_var,
                ..Default::default()
            },
        );
    }
    db
}

fn no_events_config() -> OutstationConfig {
    let mut cfg = OutstationConfig::default();
    cfg.event_buffer = EventBufferConfig::all_types(0);
    cfg
}

fn test_freeze_operation(func: &str, expect_response: bool, clear: bool) {
    let db = counter_db(1, StaticFrozenCounterVariation::Group21Var5);
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.transaction(|db| {
        db.update_counter(
            0,
            Counter::with_flags(0, Flags::new(0x01)),
            EventMode::Suppress,
        );
        db.update_frozen_counter(
            0,
            FrozenCounter::with_flags(0, Flags::new(0x01)),
            EventMode::Suppress,
        );
    });
    t.lower_layer_up();

    t.send_to_outstation(&format!("C0 {func} 14 00 06"));
    if expect_response {
        assert_eq!(t.pop_write(), "C0 81 80 00");
    } else {
        assert_eq!(t.pop_write(), "");
    }
    t.on_tx_ready();

    t.send_to_outstation("C1 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 15 05 00 00 00 01 00 00 00 00 00 00 00 00 00 00"
    );
    t.on_tx_ready();

    t.transaction(|db| {
        db.update_counter(0, Counter::new(COUNTER_VALUE), EventMode::Detect);
    });

    t.send_to_outstation("C2 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        "C2 81 80 00 15 05 00 00 00 01 00 00 00 00 00 00 00 00 00 00"
    );
    t.on_tx_ready();

    t.app.now_time = freeze_timestamp();

    t.send_to_outstation(&format!("C3 {func} 14 00 06"));
    if expect_response {
        assert_eq!(t.pop_write(), "C3 81 80 00");
    } else {
        assert_eq!(t.pop_write(), "");
    }
    t.on_tx_ready();

    t.send_to_outstation("C4 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        format!("C4 81 80 00 15 05 00 00 00 01 29 00 00 00 {FREEZE_TIMESTAMP_HEX}")
    );
    t.on_tx_ready();

    let expected_counter = if clear { "00" } else { COUNTER_VALUE_HEX };
    t.send_to_outstation("C5 01 14 00 06");
    assert_eq!(
        t.pop_write(),
        format!("C5 81 80 00 14 01 00 00 00 01 {expected_counter} 00 00 00")
    );
}

#[test]
fn freeze_0x07() {
    test_freeze_operation("07", true, false);
}

#[test]
fn freeze_no_ack_0x08() {
    test_freeze_operation("08", false, false);
}

#[test]
fn freeze_and_clear_0x09() {
    test_freeze_operation("09", true, true);
}

#[test]
fn freeze_and_clear_no_ack_0x0a() {
    test_freeze_operation("0A", false, true);
}

#[test]
fn broadcast_support() {
    let db = counter_db(1, StaticFrozenCounterVariation::Group21Var5);
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.transaction(|db| {
        db.update_counter(
            0,
            Counter::with_flags(0, Flags::new(0x01)),
            EventMode::Suppress,
        );
        db.update_frozen_counter(
            0,
            FrozenCounter::with_flags(0, Flags::new(0x01)),
            EventMode::Suppress,
        );
    });
    t.lower_layer_up();

    t.broadcast_to_outstation(LinkBroadcastAddress::DontConfirm, "C0 0A 14 00 06");
    assert_eq!(t.pop_write(), "");
    t.on_tx_ready();

    t.send_to_outstation("C1 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        "C1 81 81 00 15 05 00 00 00 01 00 00 00 00 00 00 00 00 00 00"
    );
    t.on_tx_ready();

    t.transaction(|db| {
        db.update_counter(0, Counter::new(COUNTER_VALUE), EventMode::Detect);
    });

    t.send_to_outstation("C2 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        "C2 81 80 00 15 05 00 00 00 01 00 00 00 00 00 00 00 00 00 00"
    );
    t.on_tx_ready();

    t.app.now_time = freeze_timestamp();
    t.broadcast_to_outstation(LinkBroadcastAddress::DontConfirm, "C3 0A 14 00 06");
    assert_eq!(t.pop_write(), "");
    t.on_tx_ready();

    t.send_to_outstation("C4 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        format!("C4 81 81 00 15 05 00 00 00 01 29 00 00 00 {FREEZE_TIMESTAMP_HEX}")
    );
    t.on_tx_ready();

    t.send_to_outstation("C5 01 14 00 06");
    assert_eq!(t.pop_write(), "C5 81 80 00 14 01 00 00 00 01 00 00 00 00");
}

#[test]
fn range_freeze() {
    let num_counters = 10;
    let db = counter_db(num_counters, StaticFrozenCounterVariation::Group21Var1);
    let mut t = OutstationFixture::new(no_events_config(), db);
    t.transaction(|db| {
        for i in 0..num_counters {
            db.update_counter(
                i,
                Counter::with_flags(0, Flags::new(0x01)),
                EventMode::Suppress,
            );
            db.update_frozen_counter(
                i,
                FrozenCounter::with_flags(0, Flags::new(0x01)),
                EventMode::Suppress,
            );
        }
    });
    t.lower_layer_up();

    t.send_to_outstation("C0 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 15 01 00 00 09 \
         01 00 00 00 00 01 00 00 00 00 \
         01 00 00 00 00 01 00 00 00 00 01 00 00 00 00 01 00 00 00 00 01 00 00 00 00 \
         01 00 00 00 00 01 00 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();

    t.transaction(|db| {
        for i in 0..num_counters {
            db.update_counter(i, Counter::new(COUNTER_VALUE), EventMode::Detect);
        }
    });

    t.send_to_outstation("C1 07 14 00 00 02 06");
    assert_eq!(t.pop_write(), "C1 81 80 00");
    t.on_tx_ready();

    t.send_to_outstation("C2 01 15 00 06");
    assert_eq!(
        t.pop_write(),
        "C2 81 80 00 15 01 00 00 09 \
         01 00 00 00 00 01 00 00 00 00 \
         01 29 00 00 00 01 29 00 00 00 01 29 00 00 00 01 29 00 00 00 01 29 00 00 00 \
         01 00 00 00 00 01 00 00 00 00 01 00 00 00 00"
    );
}
