//! 1:1 port of `TestStaticDataMap.cpp` (17 test cases).

use opendnp3::app::{
    ApduHandler, ApduWriter, AppControlField, Binary, HeaderInfo, IinField, Indexed, ParseMode,
    ParseResult, Range, parse_objects,
};
use opendnp3::enums::{EventMode, FunctionCode, GroupVariation, StaticBinaryVariation};
use opendnp3::outstation::{BinaryConfig, Database, DatabaseConfig, EventBufferConfig};

#[derive(Default)]
struct BinaryCollector {
    items: Vec<(GroupVariation, Indexed<Binary>)>,
}

impl ApduHandler for BinaryCollector {
    fn on_binaries(&mut self, info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        for v in values {
            self.items.push((info.gv, v.clone()));
        }
        IinField::empty()
    }
}

fn db_with_binaries(indices: &[u16]) -> Database {
    let mut cfg = DatabaseConfig::default();
    for &i in indices {
        cfg.binary.insert(i, BinaryConfig::default());
    }
    Database::new(&cfg, EventBufferConfig::all_types(10))
}

fn drain_selected_binaries(
    db: &mut Database,
    max_apdu: usize,
) -> (bool, Vec<(GroupVariation, Indexed<Binary>)>) {
    let mut w = ApduWriter::new_response(
        max_apdu,
        AppControlField::request(0),
        FunctionCode::Response,
        IinField::empty(),
    );
    let done = db.write_static(&mut w);
    let mut col = BinaryCollector::default();
    let (res, _) = parse_objects(&w.as_bytes()[4..], ParseMode::WithObjects, &mut col);
    assert_eq!(res, ParseResult::Ok);
    (done, col.items)
}

#[test]
fn update_returns_false_for_values_that_dont_exist() {
    let mut db = db_with_binaries(&[]);
    assert!(!db.update_binary(0, Binary::new(true), EventMode::Detect));
    assert!(db.events.is_empty());
}

#[test]
fn can_only_add_points_that_arent_already_defined() {
    let mut cfg = DatabaseConfig::default();
    assert!(cfg.binary.insert(0, BinaryConfig::default()).is_none());
    assert!(cfg.binary.insert(0, BinaryConfig::default()).is_some());
}

#[test]
fn can_detect_events_on_existing_point() {
    let mut db = db_with_binaries(&[0]);
    assert!(db.update_binary(0, Binary::new(true), EventMode::Detect));
    assert_eq!(db.events.len(), 1);
    assert!(db.update_binary(0, Binary::new(true), EventMode::Detect));
    assert_eq!(db.events.len(), 1);
}

#[test]
fn can_force_events_on_existing_point() {
    let mut db = db_with_binaries(&[0]);
    assert!(db.update_binary(0, Binary::new(true), EventMode::Force));
    assert!(db.select_static(GroupVariation::Group1Var1, Some(Range::new(0, 0))));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 1);
    assert!(items[0].1.value.value);
    assert_eq!(db.events.len(), 1);

    assert!(db.update_binary(0, Binary::new(true), EventMode::Force));
    assert_eq!(db.events.len(), 2);
}

#[test]
fn can_ignore_events_on_existing_point() {
    let mut db = db_with_binaries(&[0]);
    assert!(db.update_binary(0, Binary::new(true), EventMode::Suppress));
    assert!(db.select_static(GroupVariation::Group1Var1, Some(Range::new(0, 0))));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 1);
    assert!(items[0].1.value.value);
    assert_eq!(db.events.len(), 0);

    assert!(db.update_binary(0, Binary::new(true), EventMode::Suppress));
    assert_eq!(db.events.len(), 0);
}

#[test]
fn can_generate_events_on_existing_point() {
    let mut db = db_with_binaries(&[0]);
    assert!(db.update_binary(0, Binary::new(true), EventMode::EventOnly));
    assert!(db.select_static(GroupVariation::Group1Var1, Some(Range::new(0, 0))));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 1);
    assert!(!items[0].1.value.value);
    assert_eq!(db.events.len(), 1);

    assert!(db.update_binary(0, Binary::new(true), EventMode::EventOnly));
    assert!(db.select_static(GroupVariation::Group1Var1, Some(Range::new(0, 0))));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert!(!items[0].1.value.value);
    assert_eq!(db.events.len(), 2);
}

#[test]
fn can_select_all_points_using_default_variation_and_iterate() {
    let mut db = db_with_binaries(&[0, 3, 7]);
    assert!(db.select_static(GroupVariation::Group1Var0, None));
    let (done, items) = drain_selected_binaries(&mut db, 2048);
    assert!(done);
    assert_eq!(items.len(), 3);
    for (gv, _) in &items {
        assert_eq!(*gv, GroupVariation::Group1Var2);
    }
}

#[test]
fn can_select_all_points_using_specified_variation_and_iterate() {
    assert_ne!(
        BinaryConfig::default().s_var,
        StaticBinaryVariation::Group1Var1
    );
    let mut db = db_with_binaries(&[0, 3, 7]);
    assert!(db.select_static(GroupVariation::Group1Var1, None));
    let (done, items) = drain_selected_binaries(&mut db, 2048);
    assert!(done);
    assert_eq!(items.len(), 3);
    for (gv, _) in &items {
        assert_eq!(*gv, GroupVariation::Group1Var1);
    }
}

#[test]
fn can_select_a_range_and_iterate() {
    let mut db = db_with_binaries(&[1, 3, 7]);
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(0, 9)));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].1.index, 1);
    assert_eq!(items[2].1.index, 7);
}

#[test]
fn can_select_beyond_the_first_entry() {
    let mut db = db_with_binaries(&[1, 3, 7]);
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(3, 8)));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].1.index, 3);
    assert_eq!(items[1].1.index, 7);
}

#[test]
fn iterating_over_part_of_the_selection_clears_it() {
    let mut db = db_with_binaries(&[1, 3, 7]);
    assert!(db.select_static(GroupVariation::Group1Var2, None));
    // Small APDU buffer fits 2 single-point headers (4 + 6 + 6 = 16 bytes), leaving index 7
    let (done1, first) = drain_selected_binaries(&mut db, 16);
    assert!(!done1);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].1.index, 1);
    assert_eq!(first[1].1.index, 3);

    let (done2, second) = drain_selected_binaries(&mut db, 2048);
    assert!(done2);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].1.index, 7);
}

#[test]
fn iterating_over_the_entire_selection_clears_it() {
    let mut db = db_with_binaries(&[1, 3, 7]);
    assert!(db.select_static(GroupVariation::Group1Var0, None));
    let (done, first) = drain_selected_binaries(&mut db, 2048);
    assert!(done);
    assert_eq!(first.len(), 3);
    let (_, second) = drain_selected_binaries(&mut db, 2048);
    assert!(second.is_empty());
}

#[test]
fn selecting_a_range_that_doesnt_match_anything_returns_zero() {
    let mut db = db_with_binaries(&[1, 7]);
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(2, 6)));
    let (done, items) = drain_selected_binaries(&mut db, 2048);
    assert!(done);
    assert!(items.is_empty());
}

#[test]
fn selecting_the_lower_bound_only_returns_a_partial_range() {
    let mut db = db_with_binaries(&[1, 7]);
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(1, 6)));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].1.index, 1);
}

#[test]
fn selecting_the_upper_bound_only_returns_a_partial_range() {
    let mut db = db_with_binaries(&[1, 7]);
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(2, 8)));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].1.index, 7);
}

#[test]
fn selecting_beyond_the_full_range_returns_a_partial_range() {
    let mut db = db_with_binaries(&[1, 7]);
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(0, 8)));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].1.index, 1);
    assert_eq!(items[1].1.index, 7);
}

#[test]
fn can_select_disjoint_ranges_and_iterate_over_them() {
    let mut db = db_with_binaries(&[1, 2, 5, 9]);
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(1, 3)));
    db.select_static(GroupVariation::Group1Var0, Some(Range::new(6, 9)));
    let (_, items) = drain_selected_binaries(&mut db, 2048);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].1.index, 1);
    assert_eq!(items[1].1.index, 2);
    assert_eq!(items[2].1.index, 9);
}
