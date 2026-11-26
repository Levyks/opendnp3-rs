//! 1:1 port of `TestMasterAssignClass.cpp` (2 test cases).

use opendnp3::enums::{GroupVariation, MasterTaskType, PointClass, TaskCompletion};

use crate::common::{MasterFixture, no_startup_tasks};

#[test]
fn assigns_class_after_connect() {
    let mut t = MasterFixture::new(no_startup_tasks());
    t.lower_layer_up();

    t.master.assign_class_all(
        PointClass::Class1,
        GroupVariation::Group3Var0,
        t.clock.now(),
        &mut t.app,
    );

    let req = t.pop_write();
    assert!(req.ends_with(" 16 3C 02 06 03 00 06"));
    let seq = &req[..2];
    t.on_tx_ready();
    t.send_to_master(&format!("{seq} 81 00 00"));

    assert!(t.pop_write().is_empty());
    assert_eq!(
        t.app.completed.last().copied(),
        Some((MasterTaskType::AssignClass, TaskCompletion::Success))
    );
}

#[test]
fn disable_unsol_before_assign_class() {
    let mut params = no_startup_tasks();
    params.disable_unsol_on_startup = true;
    params.unsol_class_mask = opendnp3::app::ClassField::ALL_EVENT_CLASSES;
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();

    let req = t.pop_write();
    assert!(req.ends_with(" 15 3C 02 06 3C 03 06 3C 04 06"));
}
