//! 1:1 port of `TestOutstationDiscontiguousIndices.cpp` (9 test cases).

use opendnp3::app::{Binary, Flags};
use opendnp3::enums::EventMode;
use opendnp3::outstation::{BinaryConfig, DatabaseConfig, OutstationConfig};

use crate::common::OutstationFixture;

fn query_discontiguous_binary(request: &str) -> String {
    let mut db = DatabaseConfig::default();
    db.binary.insert(2, BinaryConfig::default());
    db.binary.insert(4, BinaryConfig::default());
    db.binary.insert(5, BinaryConfig::default());

    let mut t = OutstationFixture::new(OutstationConfig::default(), db);
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_binary(2, Binary::with_flags(Flags::new(0x81)), EventMode::Suppress);
        db.update_binary(4, Binary::with_flags(Flags::new(0x01)), EventMode::Suppress);
    });

    t.send_to_outstation(request);
    t.pop_write()
}

#[test]
fn read_discontiguous_class0() {
    assert_eq!(
        query_discontiguous_binary("C0 01 3C 01 06"),
        "C0 81 80 00 01 02 00 02 02 81 01 02 00 04 05 01 02"
    );
}

#[test]
fn read_discontiguous_bad_range_below() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 00 01"),
        "C0 81 80 04"
    );
}

#[test]
fn read_discontiguous_bad_range_above() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 06 09"),
        "C0 81 80 04"
    );
}

#[test]
fn read_discontiguous_single_range() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 02 02"),
        "C0 81 80 00 01 02 00 02 02 81"
    );
}

#[test]
fn read_discontiguous_double_range() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 04 05"),
        "C0 81 80 00 01 02 00 04 05 01 02"
    );
}

#[test]
fn read_discontiguous_past_upper_bound() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 05 06"),
        "C0 81 80 04 01 02 00 05 05 02"
    );
}

#[test]
fn read_discontiguous_all_data_with_multiple_ranges() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 02 02 01 02 00 04 05"),
        "C0 81 80 00 01 02 00 02 02 81 01 02 00 04 05 01 02"
    );
}

#[test]
fn read_discontiguous_all_data_with_multiple_ranges_and_range_error() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 02 03 01 02 00 04 05"),
        "C0 81 80 04 01 02 00 02 02 81 01 02 00 04 05 01 02"
    );
}

#[test]
fn read_discontiguous_all_data_with_range_error() {
    assert_eq!(
        query_discontiguous_binary("C0 01 01 02 00 02 05"),
        "C0 81 80 04 01 02 00 02 02 81 01 02 00 04 05 01 02"
    );
}
