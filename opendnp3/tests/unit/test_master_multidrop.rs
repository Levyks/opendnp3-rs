//! 1:1 port of `TestMasterMultidrop.cpp` (4 test cases).

use std::time::Duration;

use opendnp3::app::ClassField;
use opendnp3::link::Addresses;
use opendnp3::master::MasterParams;

use crate::common::{MasterFixture, no_startup_tasks};

#[test]
fn multidrop_scheduling_is_priority_based() {
    let params = MasterParams {
        disable_unsol_on_startup: false,
        ..Default::default()
    };
    let mut t1 = MasterFixture::with_addresses(params.clone(), Addresses::new(1, 10));
    let mut t2 = MasterFixture::with_addresses(params, Addresses::new(1, 11));

    t1.lower_layer_up();
    t2.lower_layer_up();

    let req1 = t1.pop_write();
    let seq1 = req1[..2].to_string();
    assert!(req1.ends_with(" 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06"));
    t1.on_tx_ready();
    t1.send_to_master(&format!("{seq1} 81 80 00"));

    // Clear restart IIN has higher priority
    let clear = t1.pop_write();
    let seq2 = clear[..2].to_string();
    assert!(clear.ends_with(" 02 50 01 00 07 07 00"));
    t1.on_tx_ready();
    t1.send_to_master(&format!("{seq2} 81 00 00"));

    let req2 = t2.pop_write();
    assert!(req2.ends_with(" 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06"));
}

#[test]
fn shutting_down_a_master_causes_2nd_master_to_run_scheduled_task() {
    let params = MasterParams {
        disable_unsol_on_startup: false,
        ..Default::default()
    };
    let mut t1 = MasterFixture::with_addresses(params.clone(), Addresses::new(1, 10));
    let mut t2 = MasterFixture::with_addresses(params, Addresses::new(1, 11));

    t1.lower_layer_up();
    t2.lower_layer_up();

    assert!(!t1.pop_write().is_empty());
    t1.on_tx_ready();
    t1.lower_layer_down();
    assert!(t1.pop_write().is_empty());

    assert!(!t2.pop_write().is_empty());
}

#[test]
fn scheduler_executes_other_sessions_tasks_if_a_session_is_timing_out() {
    let params = MasterParams::default();
    let mut t1 = MasterFixture::with_addresses(params.clone(), Addresses::new(1, 10));
    let mut t2 = MasterFixture::with_addresses(params, Addresses::new(1, 11));

    t1.lower_layer_up();
    t2.lower_layer_up();

    let r1 = t1.pop_write();
    assert!(r1.ends_with(" 15 3C 02 06 3C 03 06 3C 04 06"));
    t1.on_tx_ready();
    t1.advance_to_next_timer();

    let r2 = t2.pop_write();
    let seq = r2[..2].to_string();
    assert!(r2.ends_with(" 15 3C 02 06 3C 03 06 3C 04 06"));
    t2.on_tx_ready();
    t2.send_to_master(&format!("{seq} 81 00 00"));

    let r2_int = t2.pop_write();
    assert!(r2_int.ends_with(" 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06"));
}

#[test]
fn scheduler_still_does_integrity_polls_if_exception_scan_set_to_high_interval() {
    let mut t1 = MasterFixture::with_addresses(no_startup_tasks(), Addresses::new(1, 10));
    let mut t2 = MasterFixture::with_addresses(no_startup_tasks(), Addresses::new(1, 11));

    let now = t1.clock.now();
    t1.master
        .add_class_scan(ClassField::ALL_CLASSES, Duration::from_secs(10), now);
    t2.master.add_class_scan(
        ClassField::ALL_EVENT_CLASSES,
        Duration::from_secs(100_000),
        now,
    );
    t2.master
        .add_class_scan(ClassField::ALL_CLASSES, Duration::from_secs(15), now);

    t1.lower_layer_up();
    t2.lower_layer_up();

    t1.advance_time(Duration::from_secs(10));
    let r1 = t1.pop_write();
    let seq1 = r1[..2].to_string();
    t1.on_tx_ready();
    t1.send_to_master(&format!("{seq1} 81 00 00"));

    t2.advance_time(Duration::from_secs(15));
    let r2 = t2.pop_write();
    let seq2 = r2[..2].to_string();
    t2.on_tx_ready();
    t2.send_to_master(&format!("{seq2} 81 00 00"));
}
