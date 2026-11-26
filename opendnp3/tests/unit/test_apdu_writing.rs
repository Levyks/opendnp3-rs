//! 1:1 port of `TestAPDUWriting.cpp` (16 test cases).

use opendnp3::app::{
    Analog, ApduWriter, AppControlField, Binary, ControlRelayOutputBlock, Counter, DnpTime, Flags,
    IinField,
};
use opendnp3::enums::{
    CommandStatus, FunctionCode, GroupVariation, OperationType, QualifierCode, TripCloseCode,
};
use opendnp3::util::hex::to_hex;

fn request_writer(max_size: usize, func: FunctionCode) -> ApduWriter {
    ApduWriter::new_request(max_size, AppControlField::request(0), func)
}

fn response_writer(max_size: usize) -> ApduWriter {
    ApduWriter::new_response(
        max_size,
        AppControlField::request(0),
        FunctionCode::Response,
        IinField::empty(),
    )
}

#[test]
fn all_objects_and_rollback() {
    let mut w = request_writer(2048, FunctionCode::Read);
    assert!(w.write_all_objects(GroupVariation::Group60Var1));
    assert!(w.write_all_objects(GroupVariation::Group60Var2));
    let mark = w.mark();
    assert!(w.write_all_objects(GroupVariation::Group60Var3));
    assert!(w.write_all_objects(GroupVariation::Group60Var4));

    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 01 3C 01 06 3C 02 06 3C 03 06 3C 04 06"
    );

    w.rollback(mark);
    assert_eq!(to_hex(w.as_bytes()), "C0 01 3C 01 06 3C 02 06");
}

#[test]
fn all_objects_returns_false_when_full() {
    let mut w = request_writer(6, FunctionCode::Read);
    assert!(w.write_all_objects(GroupVariation::Group60Var1));
    assert!(!w.write_all_objects(GroupVariation::Group60Var1));
    assert_eq!(to_hex(w.as_bytes()), "C0 01 3C 01 06");
}

#[test]
fn range_write_iterator_start_stop() {
    let mut w = response_writer(2048);
    assert!(w.write_range_header_u8(GroupVariation::Group20Var6, 2, 3));
    assert!(w.write_counter(GroupVariation::Group20Var6, &Counter::new(9)));
    assert!(w.write_counter(GroupVariation::Group20Var6, &Counter::new(7)));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 81 00 00 14 06 00 02 03 09 00 07 00"
    );
}

#[test]
fn range_iterator_uint8_boundary_condition() {
    let mut w = response_writer(2048);
    assert!(w.write_range_header_u8(GroupVariation::Group1Var2, 0, 255));
    for _ in 0..256 {
        assert!(w.write_binary(GroupVariation::Group1Var2, &Binary::new(true)));
    }
    let hex = to_hex(w.as_bytes());
    assert!(hex.starts_with("C0 81 00 00 01 02 00 00 FF"));
    assert_eq!(w.as_bytes().len(), 4 + 5 + 256);
}

#[test]
fn count_iterator_uint8_boundary_condition() {
    let mut w = response_writer(2048);
    assert!(w.write_count_header_u8(GroupVariation::Group1Var2, 255));
    for _ in 0..255 {
        assert!(w.write_binary(GroupVariation::Group1Var2, &Binary::new(true)));
    }
    let hex = to_hex(w.as_bytes());
    assert!(hex.starts_with("C0 81 00 00 01 02 07 FF"));
    assert_eq!(w.as_bytes().len(), 4 + 4 + 255);
}

#[test]
fn empty_headers_when_not_enough_space_for_single_value() {
    let mut w = response_writer(8);
    let mark = w.mark();
    let ok = w.write_range_header_u8(GroupVariation::Group20Var6, 2, 2)
        && w.write_counter(GroupVariation::Group20Var6, &Counter::new(9));
    assert!(!ok);
    w.rollback(mark);
    assert_eq!(to_hex(w.as_bytes()), "C0 81 00 00");
}

#[test]
fn count_write_iterator_allows_count_of_zero() {
    let mut w = response_writer(2048);
    assert!(w.write_count_header_u16(GroupVariation::Group30Var1, 0));
    assert_eq!(to_hex(w.as_bytes()), "C0 81 00 00 1E 01 08 00 00");
}

#[test]
fn count_write_iterator_fills_up_correctly() {
    let mut w = response_writer(15);
    assert!(w.write_count_header_u8(GroupVariation::Group30Var2, 2));
    assert!(w.write_analog(
        GroupVariation::Group30Var2,
        &Analog::with_flags(9.0, Flags::new(0xFF))
    ));
    assert!(w.write_analog(
        GroupVariation::Group30Var2,
        &Analog::with_flags(7.0, Flags::new(0xFF))
    ));
    assert!(!w.write_analog(
        GroupVariation::Group30Var2,
        &Analog::with_flags(7.0, Flags::new(0xFF))
    ));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 81 00 00 1E 02 07 02 FF 09 00 FF 07 00"
    );
}

#[test]
fn prefix_write_iterator_with_single_crob() {
    let mut w = response_writer(2048);
    let crob = ControlRelayOutputBlock::new(
        OperationType::LatchOn,
        TripCloseCode::Nul,
        false,
        0x1F,
        0x10,
        0xAA,
        CommandStatus::Local,
    );
    assert!(w.write_bytes(&[12, 1, QualifierCode::Uint8CntUint8Index.to_u8(), 1, 0x21]));
    assert!(w.write_crob(&crob));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 81 00 00 0C 01 17 01 21 03 1F 10 00 00 00 AA 00 00 00 07"
    );
}

#[test]
fn prefix_write_iterator_cto() {
    let mut w = response_writer(2048);
    assert!(w.write_bytes(&[0x33, 0x01, 0x07, 0x01]));
    assert!(w.write_bytes(&DnpTime::new(0xAA).to_le_6bytes()));
    assert!(w.write_bytes(&[0x02, 0x03, 0x28, 0x02, 0x00]));
    assert!(w.write_u16(6));
    assert!(w.write_bytes(&[0x81, 0x0B, 0x00]));
    assert!(w.write_u16(7));
    assert!(w.write_bytes(&[0x81, 0x0C, 0x00]));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 81 00 00 33 01 07 01 AA 00 00 00 00 00 02 03 28 02 00 06 00 81 0B 00 07 00 81 0C 00"
    );
}

#[test]
fn prefix_write_iterator_cto_space_for_only_1_value() {
    let mut w = response_writer(26);
    assert!(w.write_bytes(&[0x33, 0x01, 0x07, 0x01]));
    assert!(w.write_bytes(&DnpTime::new(0xAA).to_le_6bytes()));
    assert!(w.write_bytes(&[0x02, 0x03, 0x28, 0x01, 0x00]));
    assert!(w.write_u16(6));
    assert!(w.write_bytes(&[0x81, 0x0B, 0x00]));
    assert!(!w.write_bytes(&[0x07, 0x00, 0x81, 0x0C, 0x00]));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 81 00 00 33 01 07 01 AA 00 00 00 00 00 02 03 28 01 00 06 00 81 0B 00"
    );
}

#[test]
fn prefix_write_iterator_not_enough_space_for_a_value() {
    let mut w = response_writer(23);
    let mark = w.mark();
    let ok = w.write_bytes(&[0x33, 0x01, 0x07, 0x01])
        && w.write_bytes(&DnpTime::new(0xAA).to_le_6bytes())
        && w.write_bytes(&[0x02, 0x03, 0x28, 0x01, 0x00])
        && w.write_u16(6)
        && w.write_bytes(&[0x81, 0x0B, 0x00]);
    assert!(!ok);
    w.rollback(mark);
    assert_eq!(to_hex(w.as_bytes()), "C0 81 00 00");
}

#[test]
fn single_value_with_index_crob_latch_on() {
    let mut w = request_writer(2048, FunctionCode::Select);
    let crob = ControlRelayOutputBlock::new(
        OperationType::LatchOn,
        TripCloseCode::Nul,
        false,
        0x1F,
        0x10,
        0xAA,
        CommandStatus::Local,
    );
    assert!(w.write_bytes(&[
        12,
        1,
        QualifierCode::Uint16Cnt.to_u8(),
        0x01,
        0x00,
        0x21,
        0x00
    ]));
    assert!(w.write_crob(&crob));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 03 0C 01 08 01 00 21 00 03 1F 10 00 00 00 AA 00 00 00 07"
    );
}

#[test]
fn single_value_with_index_crob_latch_off() {
    let mut w = request_writer(2048, FunctionCode::Select);
    let crob = ControlRelayOutputBlock::new(
        OperationType::LatchOff,
        TripCloseCode::Nul,
        false,
        0x1F,
        0x10,
        0xAA,
        CommandStatus::Local,
    );
    assert!(w.write_bytes(&[
        12,
        1,
        QualifierCode::Uint16Cnt.to_u8(),
        0x01,
        0x00,
        0x21,
        0x00
    ]));
    assert!(w.write_crob(&crob));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 03 0C 01 08 01 00 21 00 04 1F 10 00 00 00 AA 00 00 00 07"
    );
}

#[test]
fn write_single_value() {
    let mut w = request_writer(2048, FunctionCode::Write);
    assert!(w.write_count_header_u8(GroupVariation::Group50Var1, 1));
    assert!(w.write_bytes(&DnpTime::new(0x1234).to_le_6bytes()));
    assert_eq!(to_hex(w.as_bytes()), "C0 02 32 01 07 01 34 12 00 00 00 00");
}

#[test]
fn write_iin_restart() {
    let mut w = request_writer(2048, FunctionCode::Write);
    assert!(w.write_bytes(&[0x50, 0x01, 0x00, 0x07, 0x08, 0x03]));
    assert_eq!(to_hex(w.as_bytes()), "C0 02 50 01 00 07 08 03");
}
