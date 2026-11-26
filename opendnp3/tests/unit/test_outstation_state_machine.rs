//! 1:1 port of `TestOutstationStateMachine.cpp` (2 test cases).

use opendnp3::app::{Analog, Flags};
use opendnp3::enums::EventMode;
use opendnp3::outstation::{AnalogConfig, DatabaseConfig, OutstationConfig};

use crate::common::OutstationFixture;

#[test]
fn responds_to_repeat_read_request_with_same_octets_as_last_response() {
    let mut db = DatabaseConfig::default();
    db.analog.insert(0, AnalogConfig::default());
    let mut t = OutstationFixture::new(OutstationConfig::default(), db);
    t.lower_layer_up();

    t.send_to_outstation("C0 01 1E 00 06");
    assert_eq!(t.pop_write(), "C0 81 80 00 1E 01 00 00 00 02 00 00 00 00");
    t.on_tx_ready();

    t.transaction(|db| {
        db.update_analog(
            0,
            Analog::with_flags(1.0, Flags::new(0x01)),
            EventMode::Detect,
        );
    });

    t.send_to_outstation("C0 01 1E 00 06");
    assert_eq!(t.pop_write(), "C0 81 80 00 1E 01 00 00 00 02 00 00 00 00");
}

#[test]
fn responds_to_non_read_request_while_waiting_for_unsolicited_confirm() {
    let mut config = OutstationConfig::default();
    config.params.allow_unsolicited = true;
    let mut db = DatabaseConfig::default();
    db.analog.insert(0, AnalogConfig::default());
    let mut t = OutstationFixture::new(config, db);
    t.lower_layer_up();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.on_tx_ready();

    t.send_to_outstation("C0 02");
    assert_eq!(t.pop_write(), "C0 81 80 00");
    t.on_tx_ready();

    assert!(t.pop_write().is_empty());
}
