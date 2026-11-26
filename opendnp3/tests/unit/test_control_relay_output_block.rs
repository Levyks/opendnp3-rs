//! 1:1 port of `TestControlRelayOutputBlock.cpp` (5 test cases).

use opendnp3::app::ControlRelayOutputBlock;
use opendnp3::enums::{CommandStatus, OperationType, TripCloseCode};

#[test]
fn get_raw_code_from_components() {
    let crob = ControlRelayOutputBlock::new(
        OperationType::PulseOn,
        TripCloseCode::Trip,
        true,
        10,
        100,
        100,
        CommandStatus::Success,
    );
    assert_eq!(crob.raw_code, 0xA1);
}

#[test]
fn from_raw_code_preserves_the_qu_bit() {
    let crob = ControlRelayOutputBlock::from_raw_code(0x91, 10, 100, 100, CommandStatus::Success);
    assert_eq!(crob.raw_code, 0x91);
}

#[test]
fn from_raw_code_preserves_undefined_op_type() {
    let crob = ControlRelayOutputBlock::from_raw_code(0xAA, 10, 100, 100, CommandStatus::Success);
    assert_eq!(crob.raw_code, 0xAA);
    assert_eq!(crob.op_type, OperationType::Undefined);
}

#[test]
fn from_raw_code_properly_parses_the_enums() {
    let crob = ControlRelayOutputBlock::from_raw_code(0xA2, 10, 100, 100, CommandStatus::Success);
    assert_eq!(crob.op_type, OperationType::PulseOff);
    assert_eq!(crob.tcc, TripCloseCode::Trip);
    assert!(crob.clear);
}

#[test]
fn check_qu_flag() {
    let crob1 = ControlRelayOutputBlock::from_raw_code(0x51, 10, 100, 100, CommandStatus::Success);
    assert!(crob1.is_qu_bit_set());

    let crob2 = ControlRelayOutputBlock::from_raw_code(0xA1, 10, 100, 100, CommandStatus::Success);
    assert!(!crob2.is_qu_bit_set());
}
