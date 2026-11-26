//! 1:1 port of `TestLinkLayerKeepAlive.cpp` (11 test cases).

use std::time::Duration;

use opendnp3::enums::LinkFunction;
use opendnp3::link::LinkConfig;

use crate::common::{LinkLayerFixture, link_hex_request_link_status};

fn keep_alive_config(secs: u64) -> LinkConfig {
    let mut config = LinkConfig::new(true);
    config.keep_alive_timeout = Duration::from_secs(secs);
    config
}

#[test]
fn timers_activated_and_canceled_in_response_to_layer_up_down() {
    let mut t = LinkLayerFixture::default_master();
    assert!(t.link.next_wakeup().is_none());
    t.on_lower_layer_up();
    assert!(t.link.next_wakeup().is_some());
    t.on_lower_layer_down();
    assert!(t.link.next_wakeup().is_none());
}

#[test]
fn forwards_keep_alive_timeouts() {
    let mut t = LinkLayerFixture::new(keep_alive_config(5));
    t.on_lower_layer_up();

    assert!(t.link.next_wakeup().is_some());
    assert_eq!(t.num_keep_alive_transmissions, 0);

    assert!(t.advance_to_next_timer());
    assert_eq!(t.num_keep_alive_transmissions, 1);
}

#[test]
fn keep_alive_stops_on_ack() {
    let mut t = LinkLayerFixture::new(keep_alive_config(5));
    t.on_lower_layer_up();

    assert!(t.advance_to_next_timer());
    assert_eq!(
        t.pop_last_write(),
        link_hex_request_link_status(true, 1024, 1)
    );
    t.on_tx_ready();
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1024, &[]);
    assert_eq!(t.num_keep_alive_failures, 0);
}

#[test]
fn keep_alive_stops_on_nack() {
    let mut t = LinkLayerFixture::new(keep_alive_config(5));
    t.on_lower_layer_up();

    assert!(t.advance_to_next_timer());
    assert_eq!(
        t.pop_last_write(),
        link_hex_request_link_status(true, 1024, 1)
    );
    t.on_tx_ready();
    t.on_frame(LinkFunction::SecNack, false, false, false, 1, 1024, &[]);
    assert_eq!(t.num_keep_alive_failures, 0);
}

#[test]
fn keep_alive_failure_callback_is_invoked_on_timeout() {
    let config = keep_alive_config(5);
    let timeout = config.timeout;
    let mut t = LinkLayerFixture::new(config);
    t.on_lower_layer_up();

    assert!(t.advance_to_next_timer());
    assert_eq!(
        t.pop_last_write(),
        link_hex_request_link_status(true, 1024, 1)
    );
    t.on_tx_ready();
    t.advance_time(timeout);
    assert_eq!(t.num_keep_alive_failures, 1);
}

#[test]
fn keep_alive_success_callback_is_invoked_when_link_status_received() {
    let mut t = LinkLayerFixture::new(keep_alive_config(5));
    t.on_lower_layer_up();

    assert!(t.advance_to_next_timer());
    assert_eq!(
        t.pop_last_write(),
        link_hex_request_link_status(true, 1024, 1)
    );
    t.on_tx_ready();
    t.on_frame(
        LinkFunction::SecLinkStatus,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.num_keep_alive_replies, 1);
}

#[test]
fn keep_alive_is_periodic_on_failure() {
    let config = keep_alive_config(5);
    let timeout = config.timeout;
    let mut t = LinkLayerFixture::new(config);
    t.on_lower_layer_up();

    for count in 0..3 {
        assert_eq!(t.num_keep_alive_transmissions, count);
        assert!(t.advance_to_next_timer());
        assert_eq!(
            t.pop_last_write(),
            link_hex_request_link_status(true, 1024, 1)
        );
        t.on_tx_ready();
        t.advance_time(timeout);
        assert_eq!(t.num_keep_alive_failures, count + 1);
    }
}

#[test]
fn keep_alive_is_periodic_on_success() {
    let mut t = LinkLayerFixture::new(keep_alive_config(5));
    t.on_lower_layer_up();

    for count in 0..3 {
        assert_eq!(t.num_keep_alive_transmissions, count);
        assert!(t.advance_to_next_timer());
        assert_eq!(
            t.pop_last_write(),
            link_hex_request_link_status(true, 1024, 1)
        );
        t.on_tx_ready();
        t.on_frame(
            LinkFunction::SecLinkStatus,
            false,
            false,
            false,
            1,
            1024,
            &[],
        );
        assert_eq!(t.num_keep_alive_replies, count + 1);
    }
}

#[test]
fn keep_alive_success_callback_is_invoked_when_link_status_received_before_transmit_complete() {
    let mut t = LinkLayerFixture::new(keep_alive_config(5));
    t.on_lower_layer_up();

    assert!(t.advance_to_next_timer());
    assert_eq!(
        t.pop_last_write(),
        link_hex_request_link_status(true, 1024, 1)
    );
    t.on_frame(
        LinkFunction::SecLinkStatus,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    t.on_tx_ready();
    assert_eq!(t.num_keep_alive_replies, 1);
}

#[test]
fn keep_alive_timer_restarts_when_frame_is_received() {
    let mut t = LinkLayerFixture::new(keep_alive_config(5));
    t.on_lower_layer_up();

    t.advance_time(Duration::from_secs(2));
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1024, &[]);

    t.advance_time(Duration::from_secs(3));
    assert_eq!(t.pop_last_write(), "");

    t.advance_time(Duration::from_secs(2));
    assert_eq!(
        t.pop_last_write(),
        link_hex_request_link_status(true, 1024, 1)
    );
}

#[test]
fn keep_alive_disabled() {
    let mut config = LinkConfig::new(true);
    config.keep_alive_timeout = Duration::MAX;
    let mut t = LinkLayerFixture::new(config);
    t.on_lower_layer_up();

    assert_eq!(t.num_keep_alive_transmissions, 0);
    t.advance_time(Duration::from_secs(10));
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1024, &[]);

    t.advance_time(Duration::from_secs(24 * 3600));
    assert_eq!(t.num_keep_alive_transmissions, 0);
    assert_eq!(t.num_total_writes(), 0);
}
