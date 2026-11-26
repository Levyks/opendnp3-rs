//! 1:1 port of `TestOutstationAssignClass.cpp` (6 test cases).

use opendnp3::outstation::{AnalogConfig, BinaryConfig, DatabaseConfig, OutstationConfig};

use crate::common::OutstationFixture;

fn binary_db(count: u16) -> DatabaseConfig {
    let mut db = DatabaseConfig::default();
    for i in 0..count {
        db.binary.insert(i, BinaryConfig::default());
    }
    db
}

#[test]
fn rejects_with_func_not_supported_if_assign_class_not_supported() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 16 3C 03 06 01 00 06");
    assert_eq!(t.pop_write(), "C0 81 80 01");
    assert!(t.app.class_assignments.is_empty());
}

#[test]
fn rejects_with_param_error_if_type_doesnt_exist() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.app.supports_assign_class = true;
    t.lower_layer_up();

    t.send_to_outstation("C0 16 3C 03 06 01 00 06");
    assert_eq!(t.pop_write(), "C0 81 80 04");
    assert!(t.app.class_assignments.is_empty());
}

#[test]
fn accepts_assign_class_via_all_objects_header() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), binary_db(5));
    t.app.supports_assign_class = true;
    t.lower_layer_up();

    t.send_to_outstation("C0 16 3C 03 06 01 00 06");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    assert_eq!(t.app.class_assignments.len(), 1);
}

#[test]
fn rejects_assign_class_with_param_error_if_range_is_invalid() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), binary_db(5));
    t.app.supports_assign_class = true;
    t.lower_layer_up();

    t.send_to_outstation("C0 16 3C 03 06 01 00 01 00 00 05 00");
    assert_eq!(t.pop_write(), "C0 81 80 04");
}

#[test]
fn accepts_assign_class_via_start_stop() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), binary_db(5));
    t.app.supports_assign_class = true;
    t.lower_layer_up();

    t.send_to_outstation("C0 16 3C 03 06 01 00 01 02 00 03 00");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    assert_eq!(t.app.class_assignments.len(), 1);
}

#[test]
fn accepts_multiple_assignments_per_message() {
    let mut db = binary_db(5);
    for i in 0..10 {
        db.analog.insert(i, AnalogConfig::default());
    }
    let mut t = OutstationFixture::new(OutstationConfig::default(), db);
    t.app.supports_assign_class = true;
    t.lower_layer_up();

    t.send_to_outstation("C0 16 3C 03 06 01 00 01 02 00 03 00 3C 04 06 1E 00 06");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    assert_eq!(t.app.class_assignments.len(), 2);
}
