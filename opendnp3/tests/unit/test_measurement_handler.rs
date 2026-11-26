//! 1:1 port of `TestMeasurementHandler.cpp` (6 test cases).

use std::collections::HashMap;

use opendnp3::app::{
    ApduHandler, Binary, DnpTime, HeaderInfo, IinField, Indexed, ParseMode, ParseResult,
    parse_objects,
};
use opendnp3::enums::TimestampQuality;
use opendnp3::util::hex::from_hex;

#[derive(Default)]
struct MockSoe {
    binaries: HashMap<u16, Binary>,
    times: Vec<DnpTime>,
}

impl MockSoe {
    fn total_received(&self) -> usize {
        self.binaries.len() + self.times.len()
    }
}

impl ApduHandler for MockSoe {
    fn on_binaries(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        for v in values {
            self.binaries.insert(v.index, v.value);
        }
        IinField::empty()
    }

    fn on_time_g50v1(&mut self, _info: HeaderInfo, values: &[DnpTime]) -> IinField {
        self.times.extend_from_slice(values);
        IinField::empty()
    }
}

fn test_object_headers(hex: &str, expected: ParseResult, verify: impl FnOnce(&MockSoe)) {
    let bytes = from_hex(hex).unwrap();
    let mut soe = MockSoe::default();
    let (res, _) = parse_objects(&bytes, ParseMode::WithObjects, &mut soe);
    assert_eq!(res, expected);
    verify(&soe);
}

#[test]
fn accepts_empty_response() {
    test_object_headers("", ParseResult::Ok, |soe| {
        assert_eq!(soe.total_received(), 0);
    });
}

#[test]
fn parses_g50v1_correctly() {
    let objects = "32 01 07 02 AB AB AB AB AB AB BC BC BC BC BC BC";
    test_object_headers(objects, ParseResult::Ok, |soe| {
        assert_eq!(soe.total_received(), 2);
        assert_eq!(soe.times.len(), 2);
        assert_eq!(soe.times[0].value, 0xABABABABABAB);
        assert_eq!(soe.times[1].value, 0xBCBCBCBCBCBC);
    });
}

#[test]
fn parses_g2v1_with_invalid_timestamp_quality() {
    let objects = "02 01 17 01 AA 81";
    test_object_headers(objects, ParseResult::Ok, |soe| {
        let rec = soe.binaries.get(&0xAA).expect("binary at 0xAA");
        assert_eq!(rec.time.quality, TimestampQuality::Invalid);
    });
}

#[test]
fn parses_g2v2_with_synchronized_timestamp_quality() {
    let objects = "02 02 17 01 AA 81 08 00 00 00 00 00";
    test_object_headers(objects, ParseResult::Ok, |soe| {
        let rec = soe.binaries.get(&0xAA).expect("binary at 0xAA");
        assert_eq!(rec.time.value, 8);
        assert_eq!(rec.time.quality, TimestampQuality::Synchronized);
    });
}

#[test]
fn parses_g2v3_with_synchronized_cto() {
    let objects = "33 01 07 01 07 00 00 00 00 00 02 03 17 01 08 81 01 00";
    test_object_headers(objects, ParseResult::Ok, |soe| {
        let rec = soe.binaries.get(&8).expect("binary at 8");
        assert_eq!(rec.time.value, 8);
        assert_eq!(rec.time.quality, TimestampQuality::Synchronized);
    });
}

#[test]
fn parses_g2v3_with_unsynchronized_cto() {
    let objects = "33 02 07 01 07 00 00 00 00 00 02 03 17 01 08 81 01 00";
    test_object_headers(objects, ParseResult::Ok, |soe| {
        let rec = soe.binaries.get(&8).expect("binary at 8");
        assert_eq!(rec.time.value, 8);
        assert_eq!(rec.time.quality, TimestampQuality::Unsynchronized);
    });
}
