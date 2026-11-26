//! 1:1 port of `TestOutstationBroadcast.cpp` (7 test cases).

use opendnp3::app::{Binary, ClassField};
use opendnp3::enums::EventMode;
use opendnp3::link::LinkBroadcastAddress;
use opendnp3::outstation::{BinaryConfig, DatabaseConfig, EventBufferConfig, OutstationConfig};

use crate::common::OutstationFixture;

#[test]
fn shall_confirm_broadcast_asks_for_confirmation_and_resets_on_confirm() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.broadcast_to_outstation(
        LinkBroadcastAddress::ShallConfirm,
        "C0 02 50 01 00 07 07 00",
    );
    assert!(t.has_no_data());

    t.send_to_outstation("C2 01 3C 01 06");
    assert_eq!(t.pop_write(), "E2 81 01 00");
    t.on_tx_ready();

    t.send_to_outstation("C3 01 3C 01 06");
    assert_eq!(t.pop_write(), "E3 81 01 00");
    t.on_tx_ready();

    t.send_to_outstation("C3 00");
    t.on_tx_ready();

    t.send_to_outstation("C4 01 3C 01 06");
    assert_eq!(t.pop_write(), "C4 81 00 00");
}

#[test]
fn confirmation_sequence_number_is_properly_updated_even_if_response_is_not_sequence_related() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.broadcast_to_outstation(
        LinkBroadcastAddress::ShallConfirm,
        "C0 02 50 01 00 07 07 00",
    );
    assert!(t.has_no_data());

    // RecordCurrentTime (FC 0x18)
    t.send_to_outstation("C2 18");
    assert_eq!(t.pop_write(), "E2 81 01 00");
    t.on_tx_ready();

    t.send_to_outstation("C2 00");
    t.on_tx_ready();

    t.send_to_outstation("C3 01 3C 01 06");
    assert_eq!(t.pop_write(), "C3 81 00 00");
}

#[test]
fn dont_confirm_broadcast_does_not_ask_for_confirmation_but_sets_broadcast_iin_once() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.broadcast_to_outstation(LinkBroadcastAddress::DontConfirm, "C0 02 50 01 00 07 07 00");
    assert!(t.has_no_data());

    t.send_to_outstation("C2 01 3C 01 06");
    assert_eq!(t.pop_write(), "C2 81 01 00");
    t.on_tx_ready();

    t.send_to_outstation("C3 01 3C 01 06");
    assert_eq!(t.pop_write(), "C3 81 00 00");
}

#[test]
fn receiving_a_broadcast_should_assume_confirmation_failed() {
    let mut config = OutstationConfig::default();
    config.event_buffer = EventBufferConfig::all_types(10);
    let mut db = DatabaseConfig::default();
    db.binary.insert(0, BinaryConfig::default());
    let mut t = OutstationFixture::new(config, db);
    t.lower_layer_up();

    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
    });

    t.send_to_outstation("C0 01 3C 02 06");
    assert_eq!(&t.pop_write()[..11], "E0 81 80 00");
    t.on_tx_ready();

    t.broadcast_to_outstation(LinkBroadcastAddress::DontConfirm, "C1 02 50 01 00 07 07 00");
    assert!(t.has_no_data());

    t.send_to_outstation("C0 00");
    t.on_tx_ready();

    t.send_to_outstation("C2 01 3C 01 06");
    assert_eq!(&t.pop_write()[..11], "C2 81 03 00");
}

#[test]
fn broadcast_with_malformed_request_should_still_set_broadcast_iin() {
    let mut t = OutstationFixture::new(OutstationConfig::default(), DatabaseConfig::default());
    t.lower_layer_up();

    t.broadcast_to_outstation(LinkBroadcastAddress::DontConfirm, "FF FF FF");
    assert!(t.has_no_data());

    t.send_to_outstation("C2 01 3C 01 06");
    assert_eq!(t.pop_write(), "C2 81 81 00");
}

#[test]
fn unsolicited_responses_should_advertise_broadcast_iin() {
    let mut config = OutstationConfig::default();
    config.params.unsol_class_mask = ClassField::ALL_CLASSES;
    config.event_buffer = EventBufferConfig::all_types(10);
    config.params.allow_unsolicited = true;
    let mut db = DatabaseConfig::default();
    db.binary.insert(0, BinaryConfig::default());
    let mut t = OutstationFixture::new(config, db);
    t.lower_layer_up();
    t.on_tx_ready();

    assert_eq!(t.pop_write(), "F0 82 80 00");

    t.send_to_outstation("D0 00");

    t.broadcast_to_outstation(LinkBroadcastAddress::DontConfirm, "C1 02 50 01 00 07 07 00");
    assert!(t.has_no_data());

    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
    });

    assert_eq!(&t.pop_write()[..11], "F1 82 01 00");
    t.on_tx_ready();

    t.advance_to_next_timer();
    t.advance_to_next_timer();
    t.transaction(|db| {
        db.update_binary(0, Binary::new(false), EventMode::Detect);
    });

    assert_eq!(&t.pop_write()[..11], "F2 82 00 00");
}

#[test]
fn shall_confirm_unsolicited_responses_should_clear_broadcast_when_confirm_is_received() {
    let mut config = OutstationConfig::default();
    config.params.unsol_class_mask = ClassField::ALL_CLASSES;
    config.event_buffer = EventBufferConfig::all_types(10);
    config.params.allow_unsolicited = true;
    let mut db = DatabaseConfig::default();
    db.binary.insert(0, BinaryConfig::default());
    let mut t = OutstationFixture::new(config, db);
    t.lower_layer_up();
    t.on_tx_ready();

    assert_eq!(t.pop_write(), "F0 82 80 00");
    t.send_to_outstation("D0 00");

    t.broadcast_to_outstation(
        LinkBroadcastAddress::ShallConfirm,
        "C1 02 50 01 00 07 07 00",
    );
    assert!(t.has_no_data());

    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
    });
    assert_eq!(&t.pop_write()[..11], "F1 82 01 00");
    t.on_tx_ready();

    t.advance_to_next_timer();
    t.advance_to_next_timer();
    t.transaction(|db| {
        db.update_binary(0, Binary::new(false), EventMode::Detect);
    });
    assert_eq!(&t.pop_write()[..11], "F2 82 01 00");
    t.on_tx_ready();

    t.send_to_outstation("D2 00");
    t.transaction(|db| {
        db.update_binary(0, Binary::new(true), EventMode::Detect);
    });
    assert_eq!(&t.pop_write()[..11], "F3 82 00 00");
}
