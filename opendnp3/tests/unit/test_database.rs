//! 1:1 port of `TestDatabase.cpp` (14 test cases).

use opendnp3::app::{Analog, Binary, Counter, Flags};
use opendnp3::enums::{AnalogQuality, BinaryQuality, CounterQuality, EventClass, EventMode};
use opendnp3::outstation::{
    AnalogConfig, BinaryConfig, CounterConfig, Database, DatabaseConfig, EventBufferConfig,
};

fn is_analog_event(val1: f64, val2: f64, deadband: f64) -> bool {
    let mut cfg = DatabaseConfig::default();
    cfg.analog.insert(
        0,
        AnalogConfig {
            class: Some(EventClass::Ec1),
            deadband,
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_analog(0, Analog::new(val1), EventMode::Suppress);
    db.update_analog(0, Analog::new(val2), EventMode::Detect);
    !db.events.is_empty()
}

fn is_counter_event(val1: u32, val2: u32, deadband: u32) -> bool {
    let mut cfg = DatabaseConfig::default();
    cfg.counter.insert(
        0,
        CounterConfig {
            class: Some(EventClass::Ec1),
            deadband: deadband as f64,
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_counter(0, Counter::new(val1), EventMode::Suppress);
    db.update_counter(0, Counter::new(val2), EventMode::Detect);
    !db.events.is_empty()
}

#[test]
fn analog_event_zero_deadband() {
    assert!(!is_analog_event(0.0, 0.0, 0.0));
}

#[test]
fn analog_event_on_deadband() {
    assert!(!is_analog_event(0.0, 1.0, 1.0));
}

#[test]
fn analog_event_negative() {
    assert!(is_analog_event(-34.0, -36.0, 1.0));
}

#[test]
fn analog_infinite_change() {
    let max_analog = f64::MAX;
    let min_analog = -max_analog;
    assert!(is_analog_event(min_analog, max_analog, 1.0));
}

#[test]
fn counter_max_change() {
    let max = u32::MAX;
    assert!(is_counter_event(0, max, max - 1));
}

#[test]
fn analog_no_event_negative() {
    assert!(!is_analog_event(-34.0, -36.0, 2.0));
}

#[test]
fn binary_no_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.binary.insert(
        0,
        BinaryConfig {
            class: Some(EventClass::Ec1),
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_binary(
        0,
        Binary::with_flags(Flags::new(BinaryQuality::Restart as u8)),
        EventMode::Detect,
    );
    assert!(db.events.is_empty());
}

#[test]
fn analog_no_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.analog.insert(
        0,
        AnalogConfig {
            class: Some(EventClass::Ec1),
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_analog(
        0,
        Analog::with_flags(0.0, Flags::new(AnalogQuality::Restart as u8)),
        EventMode::Detect,
    );
    assert!(db.events.is_empty());
}

#[test]
fn counter_no_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.counter.insert(
        0,
        CounterConfig {
            class: Some(EventClass::Ec1),
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_counter(
        0,
        Counter::with_flags(0, Flags::new(CounterQuality::Restart as u8)),
        EventMode::Detect,
    );
    assert!(db.events.is_empty());
}

#[test]
fn binary_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.binary.insert(
        0,
        BinaryConfig {
            class: Some(EventClass::Ec1),
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_binary(
        0,
        Binary::with_flags(Flags::new(BinaryQuality::Online as u8)),
        EventMode::Detect,
    );
    assert_eq!(db.events.len(), 1);
}

#[test]
fn analog_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.analog.insert(
        0,
        AnalogConfig {
            class: Some(EventClass::Ec1),
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_analog(
        0,
        Analog::with_flags(0.0, Flags::new(AnalogQuality::Online as u8)),
        EventMode::Detect,
    );
    assert_eq!(db.events.len(), 1);
}

#[test]
fn counter_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.counter.insert(
        0,
        CounterConfig {
            class: Some(EventClass::Ec1),
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    db.update_counter(
        0,
        Counter::with_flags(0, Flags::new(CounterQuality::Online as u8)),
        EventMode::Detect,
    );
    assert_eq!(db.events.len(), 1);
}

#[test]
fn analog_last_reported_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.analog.insert(
        0,
        AnalogConfig {
            class: Some(EventClass::Ec1),
            deadband: 5.0,
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    let restart = Flags::new(AnalogQuality::Restart as u8);

    db.update_analog(0, Analog::with_flags(-2.0, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 0);
    db.update_analog(0, Analog::with_flags(5.0, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 0);
    db.update_analog(0, Analog::with_flags(6.0, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 1);
    db.update_analog(0, Analog::with_flags(1.0, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 1);
    db.update_analog(0, Analog::with_flags(-1.0, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 2);
}

#[test]
fn counter_last_reported_change() {
    let mut cfg = DatabaseConfig::default();
    cfg.counter.insert(
        0,
        CounterConfig {
            class: Some(EventClass::Ec1),
            deadband: 5.0,
            ..Default::default()
        },
    );
    let mut db = Database::new(&cfg, EventBufferConfig::all_types(10));
    let restart = Flags::new(CounterQuality::Restart as u8);

    db.update_counter(0, Counter::with_flags(1, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 0);
    db.update_counter(0, Counter::with_flags(5, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 0);
    db.update_counter(0, Counter::with_flags(6, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 1);
    db.update_counter(0, Counter::with_flags(1, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 1);
    db.update_counter(0, Counter::with_flags(0, restart), EventMode::Detect);
    assert_eq!(db.events.len(), 2);
}
