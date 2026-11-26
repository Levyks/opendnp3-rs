//! 1:1 port of `TestLinkLayer.cpp` (19 test cases).

use opendnp3::enums::LinkFunction;
use opendnp3::link::{Addresses, LinkBroadcastAddress};
use opendnp3::util::hex::to_hex;

use crate::common::{
    LinkLayerFixture, MockTransportSegment, incrementing_bytes, link_hex_ack, link_hex_link_status,
};

#[test]
fn closed_state() {
    let mut t = LinkLayerFixture::default_master();
    let segment = Box::new(MockTransportSegment::new(
        250,
        vec![0x00],
        Addresses::default(),
    ));
    assert!(!t.send(segment));
    assert!(!t.on_lower_layer_down());
    assert!(!t.on_frame(LinkFunction::SecAck, false, false, false, 1, 2, &[]));
}

#[test]
fn forwards_on_lower_layer_up() {
    let mut t = LinkLayerFixture::default_master();
    assert!(!t.upper_online);
    assert!(t.on_lower_layer_up());
    assert!(t.upper_online);
    assert!(!t.on_lower_layer_up());
}

#[test]
fn validates_master_outstation_bit() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, true, false, false, 1, 1024, &[]);
    assert_eq!(t.link.statistics.num_bad_master_bit, 1);
}

#[test]
fn validates_source_address() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1023, &[]);
    assert_eq!(t.link.statistics.num_unknown_source, 1);
}

#[test]
fn validates_destination_address() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, false, false, false, 2, 1024, &[]);
    assert_eq!(t.link.statistics.num_unknown_destination, 1);
}

#[test]
fn sec_to_pri_no_context() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1024, &[]);
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);
}

#[test]
fn unconfirmed_data_passed_up_from_idle_unreset() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    let bs = incrementing_bytes(250, 0);
    t.on_frame(
        LinkFunction::PriUnconfirmedUserData,
        false,
        false,
        false,
        1,
        1024,
        &bs,
    );
    assert_eq!(t.received_queue.len(), 1);
    assert_eq!(t.received_queue[0], to_hex(&bs));
}

#[test]
fn unconfirmed_broadcast_data_passed_up_from_idle_unreset() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    let bs = incrementing_bytes(250, 0);
    t.on_frame(
        LinkFunction::PriUnconfirmedUserData,
        false,
        false,
        false,
        LinkBroadcastAddress::ShallConfirm as u16,
        1024,
        &bs,
    );
    assert_eq!(t.received_queue.len(), 1);
    assert_eq!(t.received_queue[0], to_hex(&bs));
}

#[test]
fn confirmed_data_ignored_from_idle_unreset() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    let bs = incrementing_bytes(250, 0);
    t.on_frame(
        LinkFunction::PriConfirmedUserData,
        false,
        false,
        false,
        1,
        1024,
        &bs,
    );
    assert!(t.received_queue.is_empty());
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);
}

#[test]
fn secondary_reset_link() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 1);
    assert_eq!(t.pop_last_write(), link_hex_ack(true, false, 1024, 1));
}

#[test]
fn broadcast_secondary_reset_link() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        LinkBroadcastAddress::ShallConfirm as u16,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 0);
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);
}

#[test]
fn secondary_reset_reset_link_states() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 1);
    t.on_tx_ready();

    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 2);
    t.on_tx_ready();

    assert_eq!(t.num_total_writes(), 2);
    assert_eq!(t.pop_last_write(), link_hex_ack(true, false, 1024, 1));
}

#[test]
fn secondary_reset_confirmed_user_data() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 1);
    t.on_tx_ready();

    let bytes = incrementing_bytes(250, 0);
    t.on_frame(
        LinkFunction::PriConfirmedUserData,
        false,
        true,
        false,
        1,
        1024,
        &bytes,
    );
    assert_eq!(t.num_total_writes(), 2);
    t.on_tx_ready();

    assert_eq!(t.received_queue[0], to_hex(&bytes));
    t.received_queue.clear();

    // send with wrong FCB
    t.on_frame(
        LinkFunction::PriConfirmedUserData,
        false,
        true,
        false,
        1,
        1024,
        &bytes,
    );
    assert_eq!(t.num_total_writes(), 3); // should still get an ACK
    assert!(t.received_queue.is_empty()); // but no data
}

#[test]
fn request_status_of_link() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriRequestLinkStatus,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 1);
    t.on_tx_ready();
    assert_eq!(
        t.pop_last_write(),
        link_hex_link_status(true, false, 1024, 1)
    );

    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 2);
    t.on_tx_ready();

    t.on_frame(
        LinkFunction::PriRequestLinkStatus,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 3);
    assert_eq!(
        t.pop_last_write(),
        link_hex_link_status(true, false, 1024, 1)
    );
}

#[test]
fn broadcast_request_status_of_link() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriRequestLinkStatus,
        false,
        false,
        false,
        LinkBroadcastAddress::ShallConfirm as u16,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 0);
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);
}

#[test]
fn test_link_states() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriTestLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 0);

    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 1);
    t.on_tx_ready();

    t.on_frame(
        LinkFunction::PriTestLinkStates,
        false,
        true,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 2);
    assert_eq!(t.pop_last_write(), link_hex_ack(true, false, 1024, 1));
}

#[test]
fn broadcast_test_link_states() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriTestLinkStates,
        false,
        false,
        false,
        LinkBroadcastAddress::ShallConfirm as u16,
        1024,
        &[],
    );
    assert_eq!(t.num_total_writes(), 0);
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);
}

#[test]
fn send_unconfirmed() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();

    let segment = Box::new(MockTransportSegment::new(
        250,
        incrementing_bytes(250, 0),
        Addresses::default(),
    ));
    t.send(segment);
    assert_eq!(t.num_total_writes(), 1);
    t.on_tx_ready();

    assert_eq!(t.tx_ready_count, 1);
    assert_eq!(t.num_total_writes(), 1);
}

#[test]
fn close_behavior() {
    let mut t = LinkLayerFixture::default_master();
    t.on_lower_layer_up();

    let segments = Box::new(MockTransportSegment::new(
        250,
        incrementing_bytes(250, 0),
        Addresses::default(),
    ));
    t.send(segments);
    t.on_tx_ready();

    t.on_lower_layer_down();
    assert!(!t.upper_online);

    t.on_lower_layer_up();
    assert!(t.upper_online);
    let segments2 = Box::new(MockTransportSegment::new(
        250,
        incrementing_bytes(250, 0),
        Addresses::default(),
    ));
    t.send(segments2);
    assert_eq!(t.num_total_writes(), 2);
}
