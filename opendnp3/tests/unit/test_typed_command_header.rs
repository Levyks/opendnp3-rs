//! 1:1 port of `TestTypedCommandHeader.cpp` (5 test cases).

use opendnp3::app::{AnalogOutputInt16, AnalogOutputInt32, ApduWriter, AppControlField, Indexed};
use opendnp3::enums::FunctionCode;
use opendnp3::master::command::{CommandHeader, CommandSet, IndexQualifierMode};
use opendnp3::util::hex::to_hex;

fn write_to_hex(commands: &CommandSet, mode: IndexQualifierMode) -> String {
    let mut w = ApduWriter::new_request(
        100,
        AppControlField::request(0),
        FunctionCode::DirectOperate,
    );
    assert!(commands.write_with_mode(&mut w, mode));
    to_hex(&w.as_bytes()[2..])
}

#[test]
fn can_instantiate_a_crob_header() {
    let header = CommandHeader::Crob(Vec::new());
    let set = CommandSet {
        headers: vec![header],
    };
    assert_eq!(set.headers.len(), 1);
}

#[test]
fn formats_two_byte_qualifier_properly_if_enough_space_is_available() {
    let commands = CommandSet {
        headers: vec![CommandHeader::AnalogOutputInt16(vec![
            Indexed::new(10, AnalogOutputInt16::new(7)),
            Indexed::new(11, AnalogOutputInt16::new(8)),
        ])],
    };
    assert_eq!(
        write_to_hex(&commands, IndexQualifierMode::AlwaysTwoBytes),
        "29 02 28 02 00 0A 00 07 00 00 0B 00 08 00 00"
    );
}

#[test]
fn formats_one_byte_qualifier_properly_if_enough_space_is_available() {
    let commands = CommandSet {
        headers: vec![CommandHeader::AnalogOutputInt16(vec![
            Indexed::new(10, AnalogOutputInt16::new(7)),
            Indexed::new(11, AnalogOutputInt16::new(8)),
        ])],
    };
    assert_eq!(
        write_to_hex(&commands, IndexQualifierMode::AllowOneByte),
        "29 02 17 02 0A 07 00 00 0B 08 00 00"
    );
}

#[test]
fn does_not_format_if_insufficient_space() {
    let commands = CommandSet {
        headers: vec![CommandHeader::AnalogOutputInt16(vec![
            Indexed::new(10, AnalogOutputInt16::new(7)),
            Indexed::new(11, AnalogOutputInt16::new(8)),
        ])],
    };
    let mut w =
        ApduWriter::new_request(14, AppControlField::request(0), FunctionCode::DirectOperate);
    assert!(!commands.write_with_mode(&mut w, IndexQualifierMode::AlwaysTwoBytes));
}

#[test]
fn command_set_can_be_moved_and_written() {
    let mut commands = CommandSet::new();
    commands.add_ao_i16(10, AnalogOutputInt16::new(7));
    commands.add_ao_i32(11, AnalogOutputInt32::new(8));

    let commands2 = std::mem::take(&mut commands);

    assert!(write_to_hex(&commands, IndexQualifierMode::AlwaysTwoBytes).is_empty());
    assert_eq!(
        write_to_hex(&commands2, IndexQualifierMode::AlwaysTwoBytes),
        "29 02 28 01 00 0A 00 07 00 00 29 01 28 01 00 0B 00 08 00 00 00 00"
    );
}
