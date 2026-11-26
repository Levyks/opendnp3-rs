//! Ported C++ APDU, Flags, and CROB conformance tests:
//! - `TestAPDUParsing.cpp` (all 34 cases)
//! - `TestControlRelayOutputBlock.cpp` (all 5 cases)
//! - `TestFlags.cpp` (all 7 flag suites)
//! - `TestAPDUWriting.cpp` (mark/rollback, full-buffer, ranges, counts, CROB, CTO)

use opendnp3::app::{
    Analog, AnalogCommandEvent, ApduHandler, ApduHeader, ApduResponseHeader, ApduWriter,
    AppControlField, Binary, BinaryCommandEvent, ControlRelayOutputBlock, Counter, DnpTime,
    DoubleBitBinary, Flags, HeaderInfo, IinField, Indexed, OctetString, ParseMode, ParseResult,
    Range, parse_objects,
};
use opendnp3::enums::{
    AnalogOutputStatusQuality, AnalogQuality, BinaryOutputStatusQuality, BinaryQuality,
    CommandStatus, CounterQuality, DoubleBit, DoubleBitBinaryQuality, FrozenCounterQuality,
    FunctionCode, GroupVariation, OperationType, QualifierCode, TripCloseCode,
};
use opendnp3::util::hex::{from_hex, to_hex};

#[derive(Default)]
struct MockApduHandler {
    headers: Vec<HeaderInfo>,
    binaries: Vec<Indexed<Binary>>,
    double_bits: Vec<Indexed<DoubleBitBinary>>,
    octets: Vec<Indexed<OctetString>>,
    crobs: Vec<Indexed<ControlRelayOutputBlock>>,
    iin_bits: Vec<Indexed<bool>>,
    binary_cmd_events: Vec<Indexed<BinaryCommandEvent>>,
    analog_cmd_events: Vec<Indexed<AnalogCommandEvent>>,
    indices: Vec<u16>,
}

impl ApduHandler for MockApduHandler {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        self.headers.push(info);
        IinField::empty()
    }
    fn on_range_header(&mut self, info: HeaderInfo, _range: Range) -> IinField {
        self.headers.push(info);
        IinField::empty()
    }
    fn on_count_header(&mut self, info: HeaderInfo, _count: u16) -> IinField {
        self.headers.push(info);
        IinField::empty()
    }
    fn on_binaries(&mut self, info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        self.headers.push(info);
        self.binaries.extend_from_slice(values);
        IinField::empty()
    }
    fn on_double_bits(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<DoubleBitBinary>],
    ) -> IinField {
        self.headers.push(info);
        self.double_bits.extend_from_slice(values);
        IinField::empty()
    }
    fn on_octet_strings(&mut self, info: HeaderInfo, values: &[Indexed<OctetString>]) -> IinField {
        self.headers.push(info);
        self.octets.extend_from_slice(values);
        IinField::empty()
    }
    fn on_crobs(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<ControlRelayOutputBlock>],
    ) -> IinField {
        self.headers.push(info);
        self.crobs.extend_from_slice(values);
        IinField::empty()
    }
    fn on_iin_bits(&mut self, info: HeaderInfo, values: &[Indexed<bool>]) -> IinField {
        self.headers.push(info);
        self.iin_bits.extend_from_slice(values);
        IinField::empty()
    }
    fn on_binary_command_events(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<BinaryCommandEvent>],
    ) -> IinField {
        self.headers.push(info);
        self.binary_cmd_events.extend_from_slice(values);
        IinField::empty()
    }
    fn on_analog_command_events(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogCommandEvent>],
    ) -> IinField {
        self.headers.push(info);
        self.analog_cmd_events.extend_from_slice(values);
        IinField::empty()
    }
    fn on_indices(&mut self, info: HeaderInfo, indices: &[u16]) -> IinField {
        self.headers.push(info);
        self.indices.extend_from_slice(indices);
        IinField::empty()
    }
}

fn parse_hex(hex: &str, mode: ParseMode) -> (ParseResult, MockApduHandler) {
    let bytes = from_hex(hex).unwrap();
    let mut mock = MockApduHandler::default();
    let (res, _) = parse_objects(&bytes, mode, &mut mock);
    (res, mock)
}

// =========================================================================
// TestAPDUParsing.cpp
// =========================================================================

#[test]
fn header_parsing_empty_string() {
    assert!(ApduHeader::parse(&[]).is_none());
}

#[test]
fn header_parses_request() {
    let buf = from_hex("C0 02 AB CD").unwrap();
    let hdr = ApduHeader::parse(&buf).unwrap();
    assert_eq!(
        hdr.control.to_byte(),
        AppControlField::new(true, true, false, false, 0).to_byte()
    );
    assert_eq!(hdr.function, FunctionCode::Write);
    assert_eq!(to_hex(&buf[2..]), "AB CD");
}

#[test]
fn response_less_than_four() {
    let buf = from_hex("C0 02 01").unwrap();
    assert!(ApduResponseHeader::parse(&buf).is_none());
}

#[test]
fn header_parses_response() {
    let buf = from_hex("C0 02 01 02 BE EF").unwrap();
    let hdr = ApduResponseHeader::parse(&buf).unwrap();
    assert_eq!(hdr.function, FunctionCode::Write);
    assert_eq!(hdr.iin, IinField::new(0x01, 0x02));
    assert_eq!(to_hex(&buf[4..]), "BE EF");
}

#[test]
fn empty_string_parses_ok() {
    let (res, mock) = parse_hex("", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::Ok);
    assert!(mock.headers.is_empty());
}

#[test]
fn not_enough_data_for_header() {
    let (res, mock) = parse_hex("AB CD", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::NotEnoughDataForHeader);
    assert!(mock.headers.is_empty());
}

#[test]
fn all_objects_headers() {
    let (res, mock) = parse_hex("02 02 06 02 00 06", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.headers.len(), 2);
    assert_eq!(mock.headers[0].gv, GroupVariation::Group2Var2);
    assert_eq!(mock.headers[1].gv, GroupVariation::Group2Var0);
}

#[test]
fn unknown_qualifier() {
    let (res, _) = parse_hex("02 02 AB", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::UnknownQualifier);
}

#[test]
fn not_enough_data_for_objects() {
    let (res, _) = parse_hex("01 02 00 01 04 FF FF FF", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::NotEnoughDataForObjects);
}

#[test]
fn group1_var2_range() {
    let (res, mock) = parse_hex("01 02 00 03 05 81 01 81", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.binaries.len(), 3);
    assert_eq!(mock.binaries[0], Indexed::new(3, Binary::new(true)));
    assert_eq!(mock.binaries[1], Indexed::new(4, Binary::new(false)));
    assert_eq!(mock.binaries[2], Indexed::new(5, Binary::new(true)));
}

#[test]
fn group1_var2_range_as_read_range() {
    let (res, mock) = parse_hex("01 02 00 03 05", ParseMode::HeaderOnly);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.headers.len(), 1);
}

#[test]
fn group1_var2_count_of_zero() {
    let (res, _) = parse_hex("01 02 07 00", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::CountOfZero);
}

#[test]
fn group1_var2_with_2_headers() {
    let (res, mock) = parse_hex(
        "01 02 00 01 01 81 01 02 00 02 03 81 81",
        ParseMode::WithObjects,
    );
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.headers.len(), 2);
}

#[test]
fn group1_var2_range_16() {
    let (res, mock) = parse_hex("01 02 01 03 00 03 00 81", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.binaries, vec![Indexed::new(3, Binary::new(true))]);
}

#[test]
fn group1_var2_all_count_qualifiers() {
    for hex in ["01 02 00 00 01 81 01", "01 02 01 00 00 01 00 81 01"] {
        let (res, mock) = parse_hex(hex, ParseMode::WithObjects);
        assert_eq!(res, ParseResult::Ok);
        assert_eq!(
            mock.binaries,
            vec![
                Indexed::new(0, Binary::new(true)),
                Indexed::new(1, Binary::new(false)),
            ]
        );
    }
    let (res, _) = parse_hex("01 02 09 02 00 00 00 81 01", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::UnknownQualifier);
}

#[test]
fn flipped_range_and_unreasonable_range() {
    assert_eq!(
        parse_hex("01 02 00 05 03", ParseMode::WithObjects).0,
        ParseResult::BadStartStop
    );
    assert_eq!(
        parse_hex("01 02 00 FF 00", ParseMode::WithObjects).0,
        ParseResult::BadStartStop
    );
    assert_eq!(
        parse_hex("01 02 01 00 00 FF FF", ParseMode::WithObjects).0,
        ParseResult::NotEnoughDataForObjects
    );
}

#[test]
fn parser_rejects_empty_octet_strings() {
    let (res, mock) = parse_hex("6E 00 00 00 FF", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::InvalidObject);
    assert!(mock.headers.is_empty());
}

#[test]
fn group1_var2_count_with_index_uint8_rejected() {
    let (res, _) = parse_hex("01 02 17 01 09 81", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::InvalidObjectQualifier);
}

#[test]
fn group2_var1_count_with_all_index_sizes() {
    for hex in ["02 01 17 01 09 81", "02 01 28 01 00 09 00 81"] {
        let (res, mock) = parse_hex(hex, ParseMode::WithObjects);
        assert_eq!(res, ParseResult::Ok);
        assert_eq!(mock.binaries, vec![Indexed::new(9, Binary::new(true))]);
    }
}

#[test]
fn group1_var1_by_range() {
    let (res, mock) = parse_hex("01 01 00 03 06 09", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(
        mock.binaries,
        vec![
            Indexed::new(3, Binary::new(true)),
            Indexed::new(4, Binary::new(false)),
            Indexed::new(5, Binary::new(false)),
            Indexed::new(6, Binary::new(true)),
        ]
    );
}

#[test]
fn group12_var1_latch_on_and_latch_off() {
    let (res, mock) = parse_hex(
        "0C 01 17 01 09 03 01 64 00 00 00 C8 00 00 00 00",
        ParseMode::WithObjects,
    );
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(
        mock.crobs,
        vec![Indexed::new(
            9,
            ControlRelayOutputBlock::new(
                OperationType::LatchOn,
                TripCloseCode::Nul,
                false,
                1,
                100,
                200,
                CommandStatus::Success,
            )
        )]
    );

    let (res, mock) = parse_hex(
        "0C 01 17 01 09 04 01 64 00 00 00 C8 00 00 00 00",
        ParseMode::WithObjects,
    );
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.crobs[0].value.op_type, OperationType::LatchOff);
}

#[test]
fn test_iin_value_and_group60() {
    let (res, mock) = parse_hex("50 01 00 07 07 00", ParseMode::WithObjects);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.iin_bits, vec![Indexed::new(7, false)]);

    let (res, mock) = parse_hex(
        "3C 01 06 3C 02 06 3C 03 06 3C 04 06",
        ParseMode::WithObjects,
    );
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.headers.len(), 4);
    assert_eq!(mock.headers[0].gv, GroupVariation::Group60Var1);
    assert_eq!(mock.headers[1].gv, GroupVariation::Group60Var2);
    assert_eq!(mock.headers[2].gv, GroupVariation::Group60Var3);
    assert_eq!(mock.headers[3].gv, GroupVariation::Group60Var4);
}

#[test]
fn test_double_bit_cto() {
    let (res, mock) = parse_hex(
        "04 03 17 02 03 C1 07 00 05 41 09 00",
        ParseMode::WithObjects,
    );
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.double_bits.len(), 2);
    assert_eq!(mock.double_bits[0].index, 3);
    assert_eq!(mock.double_bits[0].value.value, DoubleBit::Indeterminate);
    assert_eq!(mock.double_bits[0].value.time.value, 7);
    assert_eq!(mock.double_bits[1].index, 5);
    assert_eq!(mock.double_bits[1].value.value, DoubleBit::DeterminedOff);
    assert_eq!(mock.double_bits[1].value.time.value, 9);
}

#[test]
fn octet_string_events_and_static() {
    let (res, mock) = parse_hex(
        "6F 05 17 02 04 68 65 6C 6C 6F FF 77 6F 72 6C 64",
        ParseMode::WithObjects,
    );
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.octets.len(), 2);
    assert_eq!(mock.octets[0].index, 4);
    assert_eq!(mock.octets[0].value.bytes, b"hello");
    assert_eq!(mock.octets[1].index, 255);
    assert_eq!(mock.octets[1].value.bytes, b"world");

    let (res, mock) = parse_hex(
        "6E 05 00 07 08 68 65 6C 6C 6F 77 6F 72 6C 64",
        ParseMode::WithObjects,
    );
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(mock.octets.len(), 2);
    assert_eq!(mock.octets[0].index, 7);
    assert_eq!(mock.octets[0].value.bytes, b"hello");
    assert_eq!(mock.octets[1].index, 8);
    assert_eq!(mock.octets[1].value.bytes, b"world");
}

#[test]
fn group13_and_group43_command_events() {
    for hex in ["0D 01 17 01 09 81", "0D 01 28 01 00 09 00 81"] {
        let (res, mock) = parse_hex(hex, ParseMode::WithObjects);
        assert_eq!(res, ParseResult::Ok);
        assert_eq!(
            mock.binary_cmd_events,
            vec![Indexed::new(
                9,
                BinaryCommandEvent::new(true, CommandStatus::Timeout, DnpTime::invalid())
            )]
        );
    }

    for hex in [
        "0D 02 17 01 09 81 88 6E D0 92 4A 01",
        "0D 02 28 01 00 09 00 81 88 6E D0 92 4A 01",
    ] {
        let (res, mock) = parse_hex(hex, ParseMode::WithObjects);
        assert_eq!(res, ParseResult::Ok);
        assert_eq!(
            mock.binary_cmd_events,
            vec![Indexed::new(
                9,
                BinaryCommandEvent::new(true, CommandStatus::Timeout, DnpTime::new(1419802341000))
            )]
        );
    }

    for hex in [
        "2B 01 17 01 09 01 32 00 00 00",
        "2B 01 28 01 00 09 00 01 32 00 00 00",
    ] {
        let (res, mock) = parse_hex(hex, ParseMode::WithObjects);
        assert_eq!(res, ParseResult::Ok);
        assert_eq!(
            mock.analog_cmd_events,
            vec![Indexed::new(
                9,
                AnalogCommandEvent::new(50.0, CommandStatus::Timeout, DnpTime::invalid())
            )]
        );
    }

    for hex in [
        "2B 03 17 01 09 01 32 00 00 00 88 6E D0 92 4A 01",
        "2B 03 28 01 00 09 00 01 32 00 00 00 88 6E D0 92 4A 01",
    ] {
        let (res, mock) = parse_hex(hex, ParseMode::WithObjects);
        assert_eq!(res, ParseResult::Ok);
        assert_eq!(
            mock.analog_cmd_events,
            vec![Indexed::new(
                9,
                AnalogCommandEvent::new(50.0, CommandStatus::Timeout, DnpTime::new(1419802341000))
            )]
        );
    }
}

#[test]
fn parse_0x17_and_0x28_as_indices_in_header_only_mode() {
    for hex in ["01 02 17 02 2A FF", "01 02 28 02 00 2A 00 FF 00"] {
        let (res, mock) = parse_hex(hex, ParseMode::HeaderOnly);
        assert_eq!(res, ParseResult::Ok);
        assert_eq!(mock.indices, vec![42, 255]);
    }
}

// =========================================================================
// TestControlRelayOutputBlock.cpp (all 5 cases)
// =========================================================================

#[test]
fn crob_all_cases() {
    // Get raw code from components
    let c1 = ControlRelayOutputBlock::new(
        OperationType::PulseOn,
        TripCloseCode::Trip,
        true,
        10,
        100,
        100,
        CommandStatus::Success,
    );
    assert_eq!(c1.raw_code, 0xA1);

    // From raw code preserves QU bit
    let c2 = ControlRelayOutputBlock::from_raw_code(0x91, 10, 100, 100, CommandStatus::Success);
    assert_eq!(c2.raw_code, 0x91);

    // From raw code preserves undefined Op Type
    let c3 = ControlRelayOutputBlock::from_raw_code(0xAA, 10, 100, 100, CommandStatus::Success);
    assert_eq!(c3.raw_code, 0xAA);
    assert_eq!(c3.op_type, OperationType::Undefined);

    // From raw code properly parses enums
    let c4 = ControlRelayOutputBlock::from_raw_code(0xA2, 10, 100, 100, CommandStatus::Success);
    assert_eq!(c4.op_type, OperationType::PulseOff);
    assert_eq!(c4.tcc, TripCloseCode::Trip);
    assert!(c4.clear);

    // Check QU flag
    assert!(
        ControlRelayOutputBlock::from_raw_code(0x51, 10, 100, 100, CommandStatus::Success)
            .is_qu_bit_set()
    );
    assert!(
        !ControlRelayOutputBlock::from_raw_code(0xA1, 10, 100, 100, CommandStatus::Success)
            .is_qu_bit_set()
    );
}

// =========================================================================
// TestFlags.cpp (all 7 measurement quality flag types)
// =========================================================================

#[test]
fn flags_all_seven_quality_types() {
    let mut f = Flags::new(0);
    f.set(BinaryQuality::Online as u8);
    f.set(BinaryQuality::State as u8);
    assert!(f.is_set_binary(BinaryQuality::Online));
    assert!(f.is_set_binary(BinaryQuality::State));
    assert!(!f.is_set_binary(BinaryQuality::CommLost));

    let f2 =
        Flags::new(DoubleBitBinaryQuality::State1 as u8 | DoubleBitBinaryQuality::Online as u8);
    assert!(f2.is_set_double_binary(DoubleBitBinaryQuality::State1));
    assert!(!f2.is_set_double_binary(DoubleBitBinaryQuality::State2));

    let f3 = Flags::new(BinaryOutputStatusQuality::LocalForced as u8);
    assert!(f3.is_set_binary_output(BinaryOutputStatusQuality::LocalForced));

    let f4 = Flags::new(CounterQuality::Rollover as u8 | CounterQuality::Discontinuity as u8);
    assert!(f4.is_set_counter(CounterQuality::Rollover));
    assert!(f4.is_set_counter(CounterQuality::Discontinuity));

    let f5 = Flags::new(FrozenCounterQuality::RemoteForced as u8);
    assert!(f5.is_set_frozen_counter(FrozenCounterQuality::RemoteForced));

    let f6 = Flags::new(AnalogQuality::OverRange as u8 | AnalogQuality::ReferenceErr as u8);
    assert!(f6.is_set_analog(AnalogQuality::OverRange));
    assert!(f6.is_set_analog(AnalogQuality::ReferenceErr));

    let f7 = Flags::new(AnalogOutputStatusQuality::OverRange as u8);
    assert!(f7.is_set_analog_output(AnalogOutputStatusQuality::OverRange));
}

// =========================================================================
// TestAPDUWriting.cpp
// =========================================================================

#[test]
fn apdu_writing_all_objects_and_rollback() {
    let mut w = ApduWriter::new_request(64, AppControlField::request(0), FunctionCode::Read);
    assert!(w.write_all_objects(GroupVariation::Group60Var1));
    assert!(w.write_all_objects(GroupVariation::Group60Var2));
    let m = w.mark();
    assert!(w.write_all_objects(GroupVariation::Group60Var3));
    assert!(w.write_all_objects(GroupVariation::Group60Var4));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 01 3C 01 06 3C 02 06 3C 03 06 3C 04 06"
    );

    w.rollback(m);
    assert_eq!(to_hex(w.as_bytes()), "C0 01 3C 01 06 3C 02 06");
}

#[test]
fn apdu_writing_returns_false_when_full() {
    let mut w = ApduWriter::new_request(6, AppControlField::request(0), FunctionCode::Read);
    assert!(w.write_all_objects(GroupVariation::Group60Var1));
    assert!(!w.write_all_objects(GroupVariation::Group60Var1));
    assert_eq!(to_hex(w.as_bytes()), "C0 01 3C 01 06");
}

#[test]
fn apdu_writing_range_counters_and_crob_wire_exact() {
    // Group20Var6 range 2..=3 with values 9, 7
    let mut w = ApduWriter::new_response(
        64,
        AppControlField::request(0),
        FunctionCode::Response,
        IinField::empty(),
    );
    assert!(w.write_range_header_u8(GroupVariation::Group20Var6, 2, 3));
    assert!(w.write_counter(GroupVariation::Group20Var6, &Counter::new(9)));
    assert!(w.write_counter(GroupVariation::Group20Var6, &Counter::new(7)));
    assert_eq!(
        to_hex(w.as_bytes()),
        "C0 81 00 00 14 06 00 02 03 09 00 07 00"
    );

    // Single CROB prefixed u8
    let mut w2 = ApduWriter::new_response(
        64,
        AppControlField::request(0),
        FunctionCode::Response,
        IinField::empty(),
    );
    assert!(w2.write_bytes(&[12, 1, QualifierCode::Uint8CntUint8Index.to_u8(), 1, 0x21]));
    let crob = ControlRelayOutputBlock::new(
        OperationType::LatchOn,
        TripCloseCode::Nul,
        false,
        0x1F,
        0x10,
        0xAA,
        CommandStatus::Local,
    );
    assert!(w2.write_crob(&crob));
    assert_eq!(
        to_hex(w2.as_bytes()),
        "C0 81 00 00 0C 01 17 01 21 03 1F 10 00 00 00 AA 00 00 00 07"
    );

    // Analog clamping & over-range flag (TestWriteConversions.cpp)
    let mut w3 = ApduWriter::new_response(
        64,
        AppControlField::request(0),
        FunctionCode::Response,
        IinField::empty(),
    );
    assert!(w3.write_range_header_u8(GroupVariation::Group30Var2, 0, 0));
    assert!(w3.write_analog(GroupVariation::Group30Var2, &Analog::new(100_000.0)));
    // Clamped to i16::MAX (0x7FFF) with OVER_RANGE (0x20) | ONLINE (0x01) = 0x21
    assert_eq!(to_hex(w3.as_bytes()), "C0 81 00 00 1E 02 00 00 00 21 FF 7F");
}
