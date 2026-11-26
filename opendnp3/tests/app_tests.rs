//! Application-layer unit tests (ported from `TestFlags.cpp`,
//! `TestControlRelayOutputBlock.cpp`, `TestAPDUParsing.cpp`, `TestAPDUWriting.cpp`).

use opendnp3::app::{
    Analog, AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    AnalogOutputStatus, ApduHandler, ApduHeader, ApduResponseHeader, ApduWriter, AppControlField,
    AppSeqNum, Binary, BinaryOutputStatus, ClassField, ControlRelayOutputBlock, Counter, DnpTime,
    DoubleBitBinary, Flags, FrozenCounter, HeaderInfo, IinBit, IinField, Indexed, OctetString,
    ParseMode, ParseResult, Range, TimeAndInterval, parse_objects,
};
use opendnp3::enums::{
    AnalogQuality, BinaryQuality, CommandStatus, DoubleBit, FunctionCode, GroupVariation,
    OperationType, QualifierCode, TimestampQuality, TripCloseCode,
};
use opendnp3::util::hex::{from_hex, to_hex};

#[derive(Default)]
struct RecordingHandler {
    all_objects: Vec<GroupVariation>,
    ranges: Vec<(GroupVariation, Range)>,
    counts: Vec<(GroupVariation, u16)>,
    binaries: Vec<Indexed<Binary>>,
    double_bits: Vec<Indexed<DoubleBitBinary>>,
    bos: Vec<Indexed<BinaryOutputStatus>>,
    counters: Vec<Indexed<Counter>>,
    frozen_counters: Vec<Indexed<FrozenCounter>>,
    analogs: Vec<Indexed<Analog>>,
    aos: Vec<Indexed<AnalogOutputStatus>>,
    octets: Vec<Indexed<OctetString>>,
    tais: Vec<Indexed<TimeAndInterval>>,
    crobs: Vec<Indexed<ControlRelayOutputBlock>>,
    ao_i32s: Vec<Indexed<AnalogOutputInt32>>,
    ao_i16s: Vec<Indexed<AnalogOutputInt16>>,
    ao_f32s: Vec<Indexed<AnalogOutputFloat32>>,
    ao_f64s: Vec<Indexed<AnalogOutputDouble64>>,
    iin_bits: Vec<Indexed<bool>>,
    times_v1: Vec<DnpTime>,
    delays: Vec<(bool, u16)>,
}

impl ApduHandler for RecordingHandler {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        self.all_objects.push(info.gv);
        IinField::empty()
    }
    fn on_range_header(&mut self, info: HeaderInfo, range: Range) -> IinField {
        self.ranges.push((info.gv, range));
        IinField::empty()
    }
    fn on_count_header(&mut self, info: HeaderInfo, count: u16) -> IinField {
        self.counts.push((info.gv, count));
        IinField::empty()
    }
    fn on_binaries(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        self.binaries.extend_from_slice(values);
        IinField::empty()
    }
    fn on_double_bits(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<DoubleBitBinary>],
    ) -> IinField {
        self.double_bits.extend_from_slice(values);
        IinField::empty()
    }
    fn on_binary_output_statuses(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<BinaryOutputStatus>],
    ) -> IinField {
        self.bos.extend_from_slice(values);
        IinField::empty()
    }
    fn on_counters(&mut self, _info: HeaderInfo, values: &[Indexed<Counter>]) -> IinField {
        self.counters.extend_from_slice(values);
        IinField::empty()
    }
    fn on_frozen_counters(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<FrozenCounter>],
    ) -> IinField {
        self.frozen_counters.extend_from_slice(values);
        IinField::empty()
    }
    fn on_analogs(&mut self, _info: HeaderInfo, values: &[Indexed<Analog>]) -> IinField {
        self.analogs.extend_from_slice(values);
        IinField::empty()
    }
    fn on_analog_output_statuses(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<AnalogOutputStatus>],
    ) -> IinField {
        self.aos.extend_from_slice(values);
        IinField::empty()
    }
    fn on_octet_strings(&mut self, _info: HeaderInfo, values: &[Indexed<OctetString>]) -> IinField {
        self.octets.extend_from_slice(values);
        IinField::empty()
    }
    fn on_time_and_intervals(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<TimeAndInterval>],
    ) -> IinField {
        self.tais.extend_from_slice(values);
        IinField::empty()
    }
    fn on_crobs(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<ControlRelayOutputBlock>],
    ) -> IinField {
        self.crobs.extend_from_slice(values);
        IinField::empty()
    }
    fn on_ao_i32s(&mut self, _info: HeaderInfo, values: &[Indexed<AnalogOutputInt32>]) -> IinField {
        self.ao_i32s.extend_from_slice(values);
        IinField::empty()
    }
    fn on_ao_i16s(&mut self, _info: HeaderInfo, values: &[Indexed<AnalogOutputInt16>]) -> IinField {
        self.ao_i16s.extend_from_slice(values);
        IinField::empty()
    }
    fn on_ao_f32s(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<AnalogOutputFloat32>],
    ) -> IinField {
        self.ao_f32s.extend_from_slice(values);
        IinField::empty()
    }
    fn on_ao_f64s(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<AnalogOutputDouble64>],
    ) -> IinField {
        self.ao_f64s.extend_from_slice(values);
        IinField::empty()
    }
    fn on_iin_bits(&mut self, _info: HeaderInfo, values: &[Indexed<bool>]) -> IinField {
        self.iin_bits.extend_from_slice(values);
        IinField::empty()
    }
    fn on_time_g50v1(&mut self, _info: HeaderInfo, values: &[DnpTime]) -> IinField {
        self.times_v1.extend_from_slice(values);
        IinField::empty()
    }
    fn on_time_delay(&mut self, _info: HeaderInfo, fine: bool, values: &[u16]) -> IinField {
        for &v in values {
            self.delays.push((fine, v));
        }
        IinField::empty()
    }
}

#[test]
fn app_control_and_iin_roundtrip() {
    let ac = AppControlField::new(true, false, true, true, 13);
    assert_eq!(AppControlField::from_byte(ac.to_byte()), ac);
    assert_eq!(ac.to_byte(), 0x80 | 0x20 | 0x10 | 13);

    let mut seq = AppSeqNum::new(15);
    seq.increment();
    assert_eq!(seq.value(), 0);

    let mut iin = IinField::empty();
    assert!(!iin.any());
    iin.set_bit(IinBit::DeviceRestart);
    iin.set_bit(IinBit::EventBufferOverflow);
    assert!(iin.is_set(IinBit::DeviceRestart));
    assert!(iin.is_set(IinBit::EventBufferOverflow));
    assert_eq!(iin.lsb, 0x80);
    assert_eq!(iin.msb, 0x08);
    iin.clear_bit(IinBit::DeviceRestart);
    assert!(!iin.is_set(IinBit::DeviceRestart));

    let hdr = ApduHeader::new(AppControlField::request(5), FunctionCode::Read);
    assert_eq!(ApduHeader::parse(&hdr.to_bytes()), Some(hdr));

    let rhdr = ApduResponseHeader::new(
        AppControlField::new(true, true, false, false, 5),
        FunctionCode::Response,
        iin,
    );
    assert_eq!(ApduResponseHeader::parse(&rhdr.to_bytes()), Some(rhdr));
}

#[test]
fn flags_and_crob_codec() {
    let f = Flags::new(BinaryQuality::Online as u8 | BinaryQuality::State as u8);
    assert!(f.is_set_binary(BinaryQuality::Online));
    assert!(f.is_set_binary(BinaryQuality::State));
    assert!(!f.is_set_binary(BinaryQuality::Restart));

    let af = Flags::new(AnalogQuality::Online as u8 | AnalogQuality::OverRange as u8);
    assert!(af.is_set_analog(AnalogQuality::OverRange));

    let crob = ControlRelayOutputBlock::new(
        OperationType::PulseOn,
        TripCloseCode::Close,
        true,
        3,
        100,
        200,
        CommandStatus::Success,
    );
    assert_eq!(crob.raw_code, 0x01 | 0x20 | 0x40);
    let wire = crob.to_bytes();
    assert_eq!(ControlRelayOutputBlock::from_bytes(&wire), Some(crob));
}

#[test]
fn parse_header_only_integrity_poll_and_ranges() {
    // Class 1, 2, 3, 0 AllObjects (standard integrity poll)
    let bytes = from_hex("3C 02 06 3C 03 06 3C 04 06 3C 01 06").unwrap();
    let mut h = RecordingHandler::default();
    let (res, _) = parse_objects(&bytes, ParseMode::HeaderOnly, &mut h);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(
        h.all_objects,
        vec![
            GroupVariation::Group60Var2,
            GroupVariation::Group60Var3,
            GroupVariation::Group60Var4,
            GroupVariation::Group60Var1,
        ]
    );

    // Range & count headers
    let bytes = from_hex("01 02 00 02 05 02 00 07 0A").unwrap();
    let mut h = RecordingHandler::default();
    let (res, _) = parse_objects(&bytes, ParseMode::HeaderOnly, &mut h);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(
        h.ranges,
        vec![(GroupVariation::Group1Var2, Range::new(2, 5))]
    );
    assert_eq!(h.counts, vec![(GroupVariation::Group2Var0, 10)]);
}

#[test]
fn parse_error_cases() {
    let mut h = RecordingHandler::default();

    // Truncated header
    assert_eq!(
        parse_objects(&[0x01, 0x02], ParseMode::HeaderOnly, &mut h).0,
        ParseResult::NotEnoughDataForHeader
    );
    // Bad start > stop
    assert_eq!(
        parse_objects(
            &[0x01, 0x02, 0x00, 0x05, 0x02],
            ParseMode::HeaderOnly,
            &mut h
        )
        .0,
        ParseResult::BadStartStop
    );
    // Count of zero
    assert_eq!(
        parse_objects(&[0x02, 0x01, 0x17, 0x00], ParseMode::WithObjects, &mut h).0,
        ParseResult::CountOfZero
    );
    // Truncated objects
    assert_eq!(
        parse_objects(
            &[0x01, 0x02, 0x00, 0x00, 0x02, 0x01],
            ParseMode::WithObjects,
            &mut h
        )
        .0,
        ParseResult::NotEnoughDataForObjects
    );
    // Unknown qualifier
    assert_eq!(
        parse_objects(&[0x01, 0x02, 0xAB], ParseMode::WithObjects, &mut h).0,
        ParseResult::UnknownQualifier
    );
}

#[test]
fn write_and_parse_roundtrip_all_major_types() {
    let mut w = ApduWriter::new_response(
        512,
        AppControlField::new(true, true, false, false, 1),
        FunctionCode::Response,
        IinField::empty(),
    );

    // 1) Packed binaries G1V1 (10 bits across 2 bytes)
    assert!(w.write_packed_binaries(
        0,
        &[
            true, false, true, true, false, false, false, true, false, true
        ]
    ));

    // 2) Counter G20V1 range 0..=1
    assert!(w.write_range_header(GroupVariation::Group20Var1, Range::new(0, 1)));
    assert!(w.write_counter(GroupVariation::Group20Var1, &Counter::new(42)));
    assert!(w.write_counter(GroupVariation::Group20Var1, &Counter::new(100_000)));

    // 3) Analog G30V5 (f32) range 5..=5
    assert!(w.write_range_header(GroupVariation::Group30Var5, Range::new(5, 5)));
    assert!(w.write_analog(GroupVariation::Group30Var5, &Analog::new(12.5)));

    // 4) DoubleBit G3V2 range 0..=1
    assert!(w.write_range_header(GroupVariation::Group3Var2, Range::new(0, 1)));
    assert!(w.write_double_bit(
        GroupVariation::Group3Var2,
        &DoubleBitBinary::new(DoubleBit::DeterminedOn)
    ));
    assert!(w.write_double_bit(
        GroupVariation::Group3Var2,
        &DoubleBitBinary::new(DoubleBit::DeterminedOff)
    ));

    // 5) CTO (G51V1) + relative-time binary event (G2V3)
    assert!(w.write_count_header_u8(GroupVariation::Group51Var1, 1));
    assert!(w.write_time48(DnpTime::new(1_000_000)));
    assert!(w.write_bytes(&[2, 3, QualifierCode::Uint8CntUint8Index.to_u8(), 1, 7]));
    assert!(w.write_binary(
        GroupVariation::Group2Var3,
        &Binary::with_flags_and_time(Flags::new(0x81), DnpTime::new(250))
    ));

    // 6) CROB (G12V1) prefixed u16
    assert!(w.write_bytes(&[12, 1, QualifierCode::Uint16CntUint16Index.to_u8()]));
    assert!(w.write_u16(1));
    assert!(w.write_u16(9));
    let crob = ControlRelayOutputBlock::new(
        OperationType::LatchOn,
        TripCloseCode::Nul,
        false,
        1,
        500,
        0,
        CommandStatus::Success,
    );
    assert!(w.write_crob(&crob));

    // 7) Analog Output Int32 (G41V1)
    assert!(w.write_bytes(&[41, 1, QualifierCode::Uint16CntUint16Index.to_u8()]));
    assert!(w.write_u16(1));
    assert!(w.write_u16(3));
    assert!(w.write_ao_i32(&AnalogOutputInt32::new(-12345)));

    // 8) Octet String (G110)
    assert!(w.write_octet_string_prefixed(110, 4, &OctetString::new(b"DNP3".to_vec())));

    // Parse back everything after the 4-byte response header
    let bytes = w.as_bytes();
    let rhdr = ApduResponseHeader::parse(bytes).unwrap();
    assert_eq!(rhdr.function, FunctionCode::Response);

    let mut h = RecordingHandler::default();
    let (res, _) = parse_objects(&bytes[4..], ParseMode::WithObjects, &mut h);
    assert_eq!(res, ParseResult::Ok);

    // 10 packed binaries + 1 CTO-timed binary event = 11
    assert_eq!(h.binaries.len(), 11);
    assert!(h.binaries[0].value.value);
    assert!(!h.binaries[1].value.value);
    assert!(h.binaries[9].value.value);
    assert_eq!(h.binaries[10].index, 7);
    assert_eq!(h.binaries[10].value.time.value, 1_000_250);
    assert_eq!(
        h.binaries[10].value.time.quality,
        TimestampQuality::Synchronized
    );

    assert_eq!(h.counters.len(), 2);
    assert_eq!(h.counters[0].value.value, 42);
    assert_eq!(h.counters[1].value.value, 100_000);

    assert_eq!(h.analogs.len(), 1);
    assert_eq!(h.analogs[0].index, 5);
    assert!((h.analogs[0].value.value - 12.5).abs() < 1e-6);

    assert_eq!(h.double_bits.len(), 2);
    assert_eq!(h.double_bits[0].value.value, DoubleBit::DeterminedOn);
    assert_eq!(h.double_bits[1].value.value, DoubleBit::DeterminedOff);

    assert_eq!(h.crobs.len(), 1);
    assert_eq!(h.crobs[0].index, 9);
    assert_eq!(h.crobs[0].value, crob);

    assert_eq!(h.ao_i32s.len(), 1);
    assert_eq!(h.ao_i32s[0].index, 3);
    assert_eq!(h.ao_i32s[0].value.value, -12345);

    assert_eq!(h.octets.len(), 1);
    assert_eq!(h.octets[0].index, 4);
    assert_eq!(h.octets[0].value.bytes, b"DNP3");
}

#[test]
fn write_iin_clear_restart_wire_format() {
    let mut w = ApduWriter::new_request(64, AppControlField::request(0), FunctionCode::Write);
    assert!(w.write_iin_bit(IinBit::DeviceRestart as u8, false));
    // Standard DNP3 "Clear Device Restart IIN" request: C0 02 50 01 00 07 07 00
    assert_eq!(to_hex(w.as_bytes()), "C0 02 50 01 00 07 07 00");

    let mut h = RecordingHandler::default();
    let (res, _) = parse_objects(&w.as_bytes()[2..], ParseMode::WithObjects, &mut h);
    assert_eq!(res, ParseResult::Ok);
    assert_eq!(h.iin_bits, vec![Indexed::new(7, false)]);
}

#[test]
fn class_field_operations() {
    let mut cf = ClassField::NONE;
    assert!(cf.is_empty());
    cf.set(ClassField::CLASS_1);
    cf.set(ClassField::CLASS_3);
    assert!(cf.has_class1());
    assert!(!cf.has_class2());
    assert!(cf.has_class3());
    assert!(cf.has_event_class());
    cf.clear(ClassField::CLASS_1);
    assert!(!cf.has_class1());
}
