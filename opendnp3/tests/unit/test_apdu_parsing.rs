//! 1:1 port of `TestAPDUParsing.cpp` (34 test cases).

use opendnp3::app::{
    AnalogCommandEvent, ApduHandler, ApduHeader, ApduResponseHeader, AppControlField, Binary,
    BinaryCommandEvent, ControlRelayOutputBlock, DnpTime, DoubleBitBinary, HeaderInfo, IinField,
    Indexed, OctetString, ParseMode, ParseResult, Range, parse_objects,
};
use opendnp3::enums::{
    CommandStatus, DoubleBit, FunctionCode, GroupVariation, OperationType, TripCloseCode,
};
use opendnp3::util::hex::{from_hex, to_hex};

#[derive(Default)]
struct MockApduHeaderHandler {
    records: Vec<HeaderInfo>,
    binaries: Vec<Indexed<Binary>>,
    double_bits: Vec<Indexed<DoubleBitBinary>>,
    octets: Vec<Indexed<OctetString>>,
    crobs: Vec<Indexed<ControlRelayOutputBlock>>,
    iin_bits: Vec<Indexed<bool>>,
    binary_cmd_events: Vec<Indexed<BinaryCommandEvent>>,
    analog_cmd_events: Vec<Indexed<AnalogCommandEvent>>,
    indices: Vec<u16>,
}

impl ApduHandler for MockApduHeaderHandler {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        self.records.push(info);
        IinField::empty()
    }
    fn on_range_header(&mut self, info: HeaderInfo, _range: Range) -> IinField {
        self.records.push(info);
        IinField::empty()
    }
    fn on_count_header(&mut self, info: HeaderInfo, _count: u16) -> IinField {
        self.records.push(info);
        IinField::empty()
    }
    fn on_binaries(&mut self, info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        self.records.push(info);
        self.binaries.extend_from_slice(values);
        IinField::empty()
    }
    fn on_double_bits(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<DoubleBitBinary>],
    ) -> IinField {
        self.records.push(info);
        self.double_bits.extend_from_slice(values);
        IinField::empty()
    }
    fn on_octet_strings(&mut self, info: HeaderInfo, values: &[Indexed<OctetString>]) -> IinField {
        self.records.push(info);
        self.octets.extend_from_slice(values);
        IinField::empty()
    }
    fn on_crobs(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<ControlRelayOutputBlock>],
    ) -> IinField {
        self.records.push(info);
        self.crobs.extend_from_slice(values);
        IinField::empty()
    }
    fn on_iin_bits(&mut self, info: HeaderInfo, values: &[Indexed<bool>]) -> IinField {
        self.records.push(info);
        self.iin_bits.extend_from_slice(values);
        IinField::empty()
    }
    fn on_binary_command_events(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<BinaryCommandEvent>],
    ) -> IinField {
        self.records.push(info);
        self.binary_cmd_events.extend_from_slice(values);
        IinField::empty()
    }
    fn on_analog_command_events(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogCommandEvent>],
    ) -> IinField {
        self.records.push(info);
        self.analog_cmd_events.extend_from_slice(values);
        IinField::empty()
    }
    fn on_indices(&mut self, info: HeaderInfo, indices: &[u16]) -> IinField {
        self.records.push(info);
        self.indices.extend_from_slice(indices);
        IinField::empty()
    }
}

fn test_complex(
    hex: &str,
    expected: ParseResult,
    num_calls: usize,
    mode: ParseMode,
    validate: impl FnOnce(&MockApduHeaderHandler),
) {
    let bytes = from_hex(hex).unwrap();
    let mut mock = MockApduHeaderHandler::default();
    let (res, _) = parse_objects(&bytes, mode, &mut mock);
    assert_eq!(res, expected);
    assert_eq!(num_calls, mock.records.len());
    validate(&mock);
}

fn test_simple(hex: &str, expected: ParseResult, num_calls: usize) {
    test_complex(hex, expected, num_calls, ParseMode::WithObjects, |_| {});
}

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
    assert_eq!(
        hdr.control.to_byte(),
        AppControlField::new(true, true, false, false, 0).to_byte()
    );
    assert_eq!(hdr.function, FunctionCode::Write);
    assert_eq!(hdr.iin, IinField::new(0x01, 0x02));
    assert_eq!(to_hex(&buf[4..]), "BE EF");
}

#[test]
fn empty_string_parses_ok() {
    test_simple("", ParseResult::Ok, 0);
}

#[test]
fn not_enough_data() {
    test_simple("AB CD", ParseResult::NotEnoughDataForHeader, 0);
}

#[test]
fn all_objects() {
    test_complex(
        "02 02 06 02 00 06",
        ParseResult::Ok,
        2,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.records[0].gv, GroupVariation::Group2Var2);
            assert_eq!(mock.records[1].gv, GroupVariation::Group2Var0);
        },
    );
}

#[test]
fn test_unknown_qualifier() {
    test_simple("02 02 AB", ParseResult::UnknownQualifier, 0);
}

#[test]
fn not_enough_data_for_objects() {
    test_simple(
        "01 02 00 01 04 FF FF FF",
        ParseResult::NotEnoughDataForObjects,
        0,
    );
}

#[test]
fn group1_var2_range() {
    test_complex(
        "01 02 00 03 05 81 01 81",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.binaries.len(), 3);
            assert_eq!(mock.binaries[0], Indexed::new(3, Binary::new(true)));
            assert_eq!(mock.binaries[1], Indexed::new(4, Binary::new(false)));
            assert_eq!(mock.binaries[2], Indexed::new(5, Binary::new(true)));
        },
    );
}

#[test]
fn group1_var2_range_as_read_range() {
    test_complex(
        "01 02 00 03 05",
        ParseResult::Ok,
        1,
        ParseMode::HeaderOnly,
        |_| {},
    );
}

#[test]
fn group1_var2_count_of_zero() {
    test_simple("01 02 07 00", ParseResult::CountOfZero, 0);
}

#[test]
fn group1_var2_with_2_headers() {
    test_complex(
        "01 02 00 01 01 81 01 02 00 02 03 81 81",
        ParseResult::Ok,
        2,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.records.len(), 2);
        },
    );
}

#[test]
fn group1_var2_range_1_to_3() {
    test_complex(
        "01 02 00 01 03 81 01 81",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.binaries.len(), 3);
            assert_eq!(mock.binaries[0], Indexed::new(1, Binary::new(true)));
            assert_eq!(mock.binaries[1], Indexed::new(2, Binary::new(false)));
            assert_eq!(mock.binaries[2], Indexed::new(3, Binary::new(true)));
        },
    );
}

#[test]
fn group1_var2_range16() {
    test_complex(
        "01 02 01 03 00 03 00 81",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.binaries.len(), 1);
            assert_eq!(mock.binaries[0], Indexed::new(3, Binary::new(true)));
        },
    );
}

#[test]
fn group1_var2_all_count_qualifiers() {
    let validate = |mock: &MockApduHeaderHandler| {
        assert_eq!(mock.binaries.len(), 2);
        assert_eq!(mock.binaries[0], Indexed::new(0, Binary::new(true)));
        assert_eq!(mock.binaries[1], Indexed::new(1, Binary::new(false)));
    };
    test_complex(
        "01 02 00 00 01 81 01",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
    test_complex(
        "01 02 01 00 00 01 00 81 01",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
    test_simple(
        "01 02 09 02 00 00 00 81 01",
        ParseResult::UnknownQualifier,
        0,
    );
}

#[test]
fn flipped_range() {
    test_simple("01 02 00 05 03", ParseResult::BadStartStop, 0);
    test_simple("01 02 00 FF 00", ParseResult::BadStartStop, 0);
}

#[test]
fn test_unreasonable_ranges() {
    test_simple(
        "01 02 01 00 00 FF FF",
        ParseResult::NotEnoughDataForObjects,
        0,
    );
}

#[test]
fn parser_does_not_allow_empty_octet_strings() {
    test_simple("6E 00 00 00 FF", ParseResult::InvalidObject, 0);
}

#[test]
fn group1_var2_count_with_index_uint8() {
    test_simple("01 02 17 01 09 81", ParseResult::InvalidObjectQualifier, 0);
}

#[test]
fn group2_var1_count_with_all_index_sizes() {
    let validate = |mock: &MockApduHeaderHandler| {
        assert_eq!(mock.binaries.len(), 1);
        assert_eq!(mock.binaries[0], Indexed::new(9, Binary::new(true)));
    };
    test_complex(
        "02 01 17 01 09 81",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
    test_complex(
        "02 01 28 01 00 09 00 81",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
}

#[test]
fn group1_var1_by_range() {
    test_complex(
        "01 01 00 03 06 09",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.binaries.len(), 4);
            assert_eq!(mock.binaries[0], Indexed::new(3, Binary::new(true)));
            assert_eq!(mock.binaries[1], Indexed::new(4, Binary::new(false)));
            assert_eq!(mock.binaries[2], Indexed::new(5, Binary::new(false)));
            assert_eq!(mock.binaries[3], Indexed::new(6, Binary::new(true)));
        },
    );
}

#[test]
fn group12_var1_with_index_sizes_latch_on() {
    let hex = "0C 01 17 01 09 03 01 64 00 00 00 C8 00 00 00 00";
    test_complex(hex, ParseResult::Ok, 1, ParseMode::WithObjects, |mock| {
        assert_eq!(mock.crobs.len(), 1);
        let crob = ControlRelayOutputBlock::new(
            OperationType::LatchOn,
            TripCloseCode::Nul,
            false,
            1,
            100,
            200,
            CommandStatus::Success,
        );
        assert_eq!(mock.crobs[0], Indexed::new(9, crob));
    });
}

#[test]
fn group12_var1_with_index_sizes_latch_off() {
    let hex = "0C 01 17 01 09 04 01 64 00 00 00 C8 00 00 00 00";
    test_complex(hex, ParseResult::Ok, 1, ParseMode::WithObjects, |mock| {
        assert_eq!(mock.crobs.len(), 1);
        let crob = ControlRelayOutputBlock::new(
            OperationType::LatchOff,
            TripCloseCode::Nul,
            false,
            1,
            100,
            200,
            CommandStatus::Success,
        );
        assert_eq!(mock.crobs[0], Indexed::new(9, crob));
    });
}

#[test]
fn test_iin_value() {
    test_complex(
        "50 01 00 07 07 00",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.iin_bits.len(), 1);
            assert!(!mock.iin_bits[0].value);
        },
    );
}

#[test]
fn group60_var1_var2_var3_var4() {
    test_complex(
        "3C 01 06 3C 02 06 3C 03 06 3C 04 06",
        ParseResult::Ok,
        4,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.records[0].gv, GroupVariation::Group60Var1);
            assert_eq!(mock.records[1].gv, GroupVariation::Group60Var2);
            assert_eq!(mock.records[2].gv, GroupVariation::Group60Var3);
            assert_eq!(mock.records[3].gv, GroupVariation::Group60Var4);
        },
    );
}

#[test]
fn test_double_bit_cto() {
    test_complex(
        "04 03 17 02 03 C1 07 00 05 41 09 00",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.double_bits.len(), 2);
            assert_eq!(mock.double_bits[0].index, 3);
            assert_eq!(mock.double_bits[0].value.value, DoubleBit::Indeterminate);
            assert_eq!(mock.double_bits[0].value.time.value, 7);
            assert_eq!(mock.double_bits[1].index, 5);
            assert_eq!(mock.double_bits[1].value.value, DoubleBit::DeterminedOff);
            assert_eq!(mock.double_bits[1].value.time.value, 9);
        },
    );
}

#[test]
fn octet_string_events() {
    test_complex(
        "6F 05 17 02 04 68 65 6C 6C 6F FF 77 6F 72 6C 64",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.octets.len(), 2);
            assert_eq!(mock.octets[0].index, 4);
            assert_eq!(mock.octets[0].value.bytes.as_slice(), b"hello");
            assert_eq!(mock.octets[1].index, 255);
            assert_eq!(mock.octets[1].value.bytes.as_slice(), b"world");
        },
    );
}

#[test]
fn octet_string_static() {
    test_complex(
        "6E 05 00 07 08 68 65 6C 6C 6F 77 6F 72 6C 64",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        |mock| {
            assert_eq!(mock.octets.len(), 2);
            assert_eq!(mock.octets[0].index, 7);
            assert_eq!(mock.octets[0].value.bytes.as_slice(), b"hello");
            assert_eq!(mock.octets[1].index, 8);
            assert_eq!(mock.octets[1].value.bytes.as_slice(), b"world");
        },
    );
}

#[test]
fn group13_var1_count_with_all_index_sizes() {
    let validate = |mock: &MockApduHeaderHandler| {
        assert_eq!(mock.binary_cmd_events.len(), 1);
        assert_eq!(
            mock.binary_cmd_events[0],
            Indexed::new(
                9,
                BinaryCommandEvent::new(true, CommandStatus::Timeout, DnpTime::invalid()),
            )
        );
    };
    test_complex(
        "0D 01 17 01 09 81",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
    test_complex(
        "0D 01 28 01 00 09 00 81",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
}

#[test]
fn group13_var2_count_with_all_index_sizes() {
    let validate = |mock: &MockApduHeaderHandler| {
        assert_eq!(mock.binary_cmd_events.len(), 1);
        assert_eq!(
            mock.binary_cmd_events[0],
            Indexed::new(
                9,
                BinaryCommandEvent::new(true, CommandStatus::Timeout, DnpTime::new(1419802341000))
            )
        );
    };
    test_complex(
        "0D 02 17 01 09 81 88 6E D0 92 4A 01",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
    test_complex(
        "0D 02 28 01 00 09 00 81 88 6E D0 92 4A 01",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
}

#[test]
fn group43_var1_count_with_all_index_sizes() {
    let validate = |mock: &MockApduHeaderHandler| {
        assert_eq!(mock.analog_cmd_events.len(), 1);
        assert_eq!(
            mock.analog_cmd_events[0],
            Indexed::new(
                9,
                AnalogCommandEvent::new(50.0, CommandStatus::Timeout, DnpTime::invalid()),
            )
        );
    };
    test_complex(
        "2B 01 17 01 09 01 32 00 00 00",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
    test_complex(
        "2B 01 28 01 00 09 00 01 32 00 00 00",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
}

#[test]
fn group43_var3_count_with_all_index_sizes() {
    let validate = |mock: &MockApduHeaderHandler| {
        assert_eq!(mock.analog_cmd_events.len(), 1);
        assert_eq!(
            mock.analog_cmd_events[0],
            Indexed::new(
                9,
                AnalogCommandEvent::new(50.0, CommandStatus::Timeout, DnpTime::new(1419802341000))
            )
        );
    };
    test_complex(
        "2B 03 17 01 09 01 32 00 00 00 88 6E D0 92 4A 01",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
    test_complex(
        "2B 03 28 01 00 09 00 01 32 00 00 00 88 6E D0 92 4A 01",
        ParseResult::Ok,
        1,
        ParseMode::WithObjects,
        validate,
    );
}

#[test]
fn can_parse_0x17_and_0x28_qualifiers_as_collection_of_indices() {
    let validate = |mock: &MockApduHeaderHandler| {
        assert_eq!(mock.indices, vec![42, 255]);
    };
    test_complex(
        "01 02 17 02 2A FF",
        ParseResult::Ok,
        1,
        ParseMode::HeaderOnly,
        validate,
    );
    test_complex(
        "01 02 28 02 00 2A 00 FF 00",
        ParseResult::Ok,
        1,
        ParseMode::HeaderOnly,
        validate,
    );
}
