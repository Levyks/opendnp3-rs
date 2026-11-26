//! 1:1 port of `TestOutstationCommandResponses.cpp` (24 test cases).

use std::time::Duration;

use opendnp3::enums::CommandStatus;
use opendnp3::outstation::{DatabaseConfig, OutstationConfig};

use crate::common::OutstationFixture;

#[test]
fn select_crob_not_supported() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.cmd.crob_select_status = Some(CommandStatus::NotSupported);

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 04"
    );
}

#[test]
fn unknown_code_is_echoed() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.cmd.crob_select_status = Some(CommandStatus::NotSupported);

    t.send_to_outstation("C0 03 0C 01 17 01 03 AA 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 04 0C 01 17 01 03 AA 01 01 00 00 00 01 00 00 00 04"
    );
}

#[test]
fn crob_with_qu_set_returns_not_supported() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 03 0C 01 17 01 03 51 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 04 0C 01 17 01 03 51 01 01 00 00 00 01 00 00 00 04"
    );
    assert_eq!(t.cmd.selects + t.cmd.operates, 0);
}

#[test]
fn select_crob_too_many() {
    let mut cfg = OutstationConfig::default();
    cfg.params.max_controls_per_request = 1;
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation(
        "C0 03 0C 01 17 02 03 01 01 01 00 00 00 01 00 00 00 00 04 01 01 01 00 00 00 01 00 00 00 00",
    );
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 02 03 01 01 01 00 00 00 01 00 00 00 00 04 01 01 01 00 00 00 01 00 00 00 08"
    );
}

#[test]
fn select_operate_crob() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();

    t.send_to_outstation("C1 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
}

#[test]
fn select_retry_and_operate_crob() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();

    t.send_to_outstation("C1 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
}

#[test]
fn select_operate_timeout() {
    let mut cfg = OutstationConfig::default();
    cfg.params.select_timeout = Duration::from_secs(5);
    let mut t = OutstationFixture::new(cfg, DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();

    t.advance_time(Duration::from_millis(5001));

    t.send_to_outstation("C1 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 01"
    );
}

#[test]
fn select_operate_gap_in_sequence_number() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    assert_eq!(t.cmd.selects + t.cmd.operates, 1);
    t.on_tx_ready();

    t.send_to_outstation("C2 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C2 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 02"
    );
    assert_eq!(t.cmd.selects + t.cmd.operates, 1);
    t.on_tx_ready();

    t.send_to_outstation("C1 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 02"
    );
    assert_eq!(t.cmd.selects + t.cmd.operates, 1);
}

#[test]
fn select_operate_same_sequence_number() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();

    t.send_to_outstation("C0 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 02"
    );
}

#[test]
fn select_operate_non_matching_requests() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    t.on_tx_ready();

    t.send_to_outstation("C1 04 0C 01 17 01 04 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 0C 01 17 01 04 01 01 01 00 00 00 01 00 00 00 02"
    );
}

#[test]
fn select_operate_crob_same_sequence_number() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    assert_eq!(t.cmd.selects + t.cmd.operates, 0);

    t.send_to_outstation("C0 03 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    assert_eq!(t.cmd.selects + t.cmd.operates, 1);
    t.on_tx_ready();

    t.send_to_outstation("C1 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    assert_eq!(t.cmd.selects + t.cmd.operates, 2);
    t.on_tx_ready();

    t.send_to_outstation("C1 04 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00"
    );
    assert_eq!(t.cmd.selects + t.cmd.operates, 2);
}

#[test]
fn select_group41_var1() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 01 17 01 03 00 00 00 00 00");
    assert_eq!(t.pop_write(), "C0 81 80 00 29 01 17 01 03 00 00 00 00 00");
}

#[test]
fn select_group41_var2() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 02 17 01 03 00 00 00");
    assert_eq!(t.pop_write(), "C0 81 80 00 29 02 17 01 03 00 00 00");
}

#[test]
fn select_group41_var3() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 03 17 01 01 00 00 C8 42 00");
    assert_eq!(t.pop_write(), "C0 81 80 00 29 03 17 01 01 00 00 C8 42 00");
}

#[test]
fn select_group41_var4() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 04 17 01 01 00 00 00 00 00 00 59 40 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 29 04 17 01 01 00 00 00 00 00 00 59 40 00"
    );
}

#[test]
fn select_operate_group41_var1() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 01 17 01 03 00 00 00 00 00");
    assert_eq!(t.pop_write(), "C0 81 80 00 29 01 17 01 03 00 00 00 00 00");
    t.on_tx_ready();
    t.send_to_outstation("C1 04 29 01 17 01 03 00 00 00 00 00");
    assert_eq!(t.pop_write(), "C1 81 80 00 29 01 17 01 03 00 00 00 00 00");
}

#[test]
fn select_operate_group41_var2() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 02 17 01 03 00 00 00");
    assert_eq!(t.pop_write(), "C0 81 80 00 29 02 17 01 03 00 00 00");
    t.on_tx_ready();
    t.send_to_outstation("C1 04 29 02 17 01 03 00 00 00");
    assert_eq!(t.pop_write(), "C1 81 80 00 29 02 17 01 03 00 00 00");
}

#[test]
fn select_operate_group41_var3() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 03 17 01 01 00 00 C8 42 00");
    assert_eq!(t.pop_write(), "C0 81 80 00 29 03 17 01 01 00 00 C8 42 00");
    t.on_tx_ready();
    t.send_to_outstation("C1 04 29 03 17 01 01 00 00 C8 42 00");
    assert_eq!(t.pop_write(), "C1 81 80 00 29 03 17 01 01 00 00 C8 42 00");
}

#[test]
fn select_operate_group41_var4() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C0 03 29 04 17 01 01 00 00 00 00 00 00 59 40 00");
    assert_eq!(
        t.pop_write(),
        "C0 81 80 00 29 04 17 01 01 00 00 00 00 00 00 59 40 00"
    );
    t.on_tx_ready();
    t.send_to_outstation("C1 04 29 04 17 01 01 00 00 00 00 00 00 59 40 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 29 04 17 01 01 00 00 00 00 00 00 59 40 00"
    );
}

#[test]
fn direct_operate_no_response_group12_var1() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    let req = "C1 06 0C 01 17 01 03 01 01 01 00 00 00 01 00 00 00 00";
    for i in 1..5 {
        t.send_to_outstation(req);
        assert!(t.pop_write().is_empty());
        assert_eq!(t.cmd.operates, i);
    }
}

#[test]
fn direct_operate_group41_var1() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C1 05 29 01 17 01 03 00 00 00 00 00");
    assert_eq!(t.pop_write(), "C1 81 80 00 29 01 17 01 03 00 00 00 00 00");
}

#[test]
fn direct_operate_group41_var2() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C1 05 29 02 17 01 03 00 00 00");
    assert_eq!(t.pop_write(), "C1 81 80 00 29 02 17 01 03 00 00 00");
}

#[test]
fn direct_operate_group41_var3() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C1 05 29 03 17 01 01 00 00 C7 42 00");
    assert_eq!(t.pop_write(), "C1 81 80 00 29 03 17 01 01 00 00 C7 42 00");
    assert_eq!(t.cmd.ao_f32.len(), 1);
    assert_eq!(t.cmd.ao_f32[0].0, 1);
    assert!((t.cmd.ao_f32[0].1 - 99.5).abs() < 1e-6);
}

#[test]
fn direct_operate_group41_var4() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();
    t.send_to_outstation("C1 05 29 04 17 01 01 00 00 00 00 00 00 58 40 00");
    assert_eq!(
        t.pop_write(),
        "C1 81 80 00 29 04 17 01 01 00 00 00 00 00 00 58 40 00"
    );
    assert_eq!(t.cmd.ao_f64.len(), 1);
    assert_eq!(t.cmd.ao_f64[0].0, 1);
    assert!((t.cmd.ao_f64[0].1 - 96.0).abs() < 1e-6);
}
