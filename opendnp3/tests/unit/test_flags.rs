//! 1:1 port of `TestFlags.cpp` (7 test cases).

use opendnp3::app::Flags;
use opendnp3::enums::{
    AnalogOutputStatusQuality, AnalogQuality, BinaryOutputStatusQuality, BinaryQuality,
    CounterQuality, DoubleBitBinaryQuality, FrozenCounterQuality,
};

#[test]
fn binary_quality() {
    let mut flags = Flags::new(0);
    for b in [
        BinaryQuality::Online,
        BinaryQuality::Restart,
        BinaryQuality::CommLost,
        BinaryQuality::RemoteForced,
        BinaryQuality::LocalForced,
        BinaryQuality::ChatterFilter,
        BinaryQuality::Reserved,
        BinaryQuality::State,
    ] {
        flags.set(b as u8);
        assert!(flags.is_set_binary(b));
    }
    assert_eq!(flags.value, 0xFF);
}

#[test]
fn double_bit_binary_quality() {
    let mut flags = Flags::new(0);
    for b in [
        DoubleBitBinaryQuality::Online,
        DoubleBitBinaryQuality::Restart,
        DoubleBitBinaryQuality::CommLost,
        DoubleBitBinaryQuality::RemoteForced,
        DoubleBitBinaryQuality::LocalForced,
        DoubleBitBinaryQuality::ChatterFilter,
        DoubleBitBinaryQuality::State1,
        DoubleBitBinaryQuality::State2,
    ] {
        flags.set(b as u8);
        assert!(flags.is_set_double_binary(b));
    }
    assert_eq!(flags.value, 0xFF);
}

#[test]
fn analog_quality() {
    let mut flags = Flags::new(0);
    for b in [
        AnalogQuality::Online,
        AnalogQuality::Restart,
        AnalogQuality::CommLost,
        AnalogQuality::RemoteForced,
        AnalogQuality::LocalForced,
        AnalogQuality::OverRange,
        AnalogQuality::ReferenceErr,
        AnalogQuality::Reserved,
    ] {
        flags.set(b as u8);
        assert!(flags.is_set_analog(b));
    }
    assert_eq!(flags.value, 0xFF);
}

#[test]
fn counter_quality() {
    let mut flags = Flags::new(0);
    for b in [
        CounterQuality::Online,
        CounterQuality::Restart,
        CounterQuality::CommLost,
        CounterQuality::RemoteForced,
        CounterQuality::LocalForced,
        CounterQuality::Rollover,
        CounterQuality::Discontinuity,
        CounterQuality::Reserved,
    ] {
        flags.set(b as u8);
        assert!(flags.is_set_counter(b));
    }
    assert_eq!(flags.value, 0xFF);
}

#[test]
fn frozen_counter_quality() {
    let mut flags = Flags::new(0);
    for b in [
        FrozenCounterQuality::Online,
        FrozenCounterQuality::Restart,
        FrozenCounterQuality::CommLost,
        FrozenCounterQuality::RemoteForced,
        FrozenCounterQuality::LocalForced,
        FrozenCounterQuality::Rollover,
        FrozenCounterQuality::Discontinuity,
        FrozenCounterQuality::Reserved,
    ] {
        flags.set(b as u8);
        assert!(flags.is_set_frozen_counter(b));
    }
    assert_eq!(flags.value, 0xFF);
}

#[test]
fn binary_output_status_quality() {
    let mut flags = Flags::new(0);
    for b in [
        BinaryOutputStatusQuality::Online,
        BinaryOutputStatusQuality::Restart,
        BinaryOutputStatusQuality::CommLost,
        BinaryOutputStatusQuality::RemoteForced,
        BinaryOutputStatusQuality::LocalForced,
        BinaryOutputStatusQuality::Reserved1,
        BinaryOutputStatusQuality::Reserved2,
        BinaryOutputStatusQuality::State,
    ] {
        flags.set(b as u8);
        assert!(flags.is_set_binary_output(b));
    }
    assert_eq!(flags.value, 0xFF);
}

#[test]
fn analog_output_status_quality() {
    let mut flags = Flags::new(0);
    for b in [
        AnalogOutputStatusQuality::Online,
        AnalogOutputStatusQuality::Restart,
        AnalogOutputStatusQuality::CommLost,
        AnalogOutputStatusQuality::RemoteForced,
        AnalogOutputStatusQuality::LocalForced,
        AnalogOutputStatusQuality::OverRange,
        AnalogOutputStatusQuality::ReferenceErr,
        AnalogOutputStatusQuality::Reserved,
    ] {
        flags.set(b as u8);
        assert!(flags.is_set_analog_output(b));
    }
    assert_eq!(flags.value, 0xFF);
}
