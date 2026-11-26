//! 1:1 port of `TestMasterUnsolBehaviors.cpp` (1 test case).

use opendnp3::master::MasterParams;

use crate::common::MasterFixture;

#[test]
fn receive_unsol_before_transmit() {
    let params = MasterParams {
        disable_unsol_on_startup: false,
        ..Default::default()
    };
    let mut t = MasterFixture::new(params);
    t.lower_layer_up();

    t.send_to_master("F0 82 00 00");

    // Master sends unsolicited confirm and integrity poll
    let first = t.pop_write();
    t.on_tx_ready();
    let second = t.pop_write();
    assert!(!first.is_empty());
    assert!(first == "D0 00" || second == "D0 00");
}
