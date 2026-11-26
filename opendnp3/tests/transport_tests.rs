//! Ported unit tests for the DNP3 transport layer (`TestTransportLayer.cpp`).

use opendnp3::Message;
use opendnp3::clock::ManualClock;
use opendnp3::link::{Addresses, LinkConfig, LinkEvent, LinkLayer, TransportSegment};
use opendnp3::transport::{
    MAX_TPDU_PAYLOAD, TransportEvent, TransportHeader, TransportLayer, TransportTx,
};
use opendnp3::util::hex::{from_hex, to_hex};

const DEFAULT_MAX_APDU_SIZE: usize = 2048;

struct TransportFixture {
    clock: ManualClock,
    link: LinkLayer,
    transport: TransportLayer,
    events: Vec<TransportEvent>,
}

impl TransportFixture {
    fn new(start_online: bool, max_rx_frag_size: usize) -> Self {
        let mut f = Self {
            clock: ManualClock::new(),
            link: LinkLayer::new(LinkConfig::new(true)),
            transport: TransportLayer::new(max_rx_frag_size),
            events: Vec::new(),
        };
        if start_online {
            f.lower_layer_up();
        }
        f
    }

    fn lower_layer_up(&mut self) {
        self.link.on_lower_layer_up(self.clock.now());
        self.pump_link_events();
    }

    fn lower_layer_down(&mut self) {
        self.link.on_lower_layer_down();
        self.pump_link_events();
    }

    fn pump_link_events(&mut self) {
        for ev in self.link.drain_events() {
            self.transport.handle_link_event(&ev);
        }
        self.events.extend(self.transport.drain_events());
    }

    fn send_up_hex(&mut self, hex: &str) {
        let bytes = from_hex(hex).expect("valid hex");
        self.transport
            .handle_link_event(&LinkEvent::Message(Message::new(
                Addresses::new(1024, 1),
                bytes,
            )));
        self.events.extend(self.transport.drain_events());
    }

    fn send_down_hex(&mut self, hex: &str) -> bool {
        let bytes = from_hex(hex).expect("valid hex");
        let ok = self.transport.begin_transmit(
            Message::new(Addresses::new(1, 1024), bytes),
            &mut self.link,
            self.clock.now(),
        );
        self.pump_link_events();
        ok
    }

    fn received_messages(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|e| match e {
                TransportEvent::Message(m) => Some(to_hex(&m.payload)),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn repeat_sends_do_not_change_statistics() {
    let mut tx = TransportTx::new(Message::new(
        Addresses::default(),
        from_hex("12 34 56").unwrap(),
    ));
    let seg1 = to_hex(tx.segment());
    assert_eq!(seg1, "C0 12 34 56");
    assert_eq!(tx.statistics.num_transport_tx, 1);

    let seg2 = to_hex(tx.segment());
    assert_eq!(seg2, "C0 12 34 56");
    assert_eq!(tx.statistics.num_transport_tx, 1);
}

#[test]
fn state_offline_and_ready() {
    let mut f = TransportFixture::new(false, DEFAULT_MAX_APDU_SIZE);
    assert!(!f.transport.is_online());
    assert!(!f.send_down_hex("00"));

    f.lower_layer_up();
    assert!(f.transport.is_online());
    f.lower_layer_down();
    assert!(!f.transport.is_online());
    f.lower_layer_up();
    assert!(f.transport.is_online());
}

#[test]
fn allows_header_only_final_frame() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    f.send_up_hex("41 DE AD BE EF"); // FIR=1, FIN=0, SEQ=1
    f.send_up_hex("82"); // FIR=0, FIN=1, SEQ=2
    assert_eq!(f.received_messages(), vec!["DE AD BE EF"]);
}

#[test]
fn receive_no_fir() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    f.send_up_hex("80 77"); // FIR=0, FIN=1
    assert_eq!(f.transport.rx.statistics.num_transport_ignore, 1);
    assert!(f.received_messages().is_empty());
}

#[test]
fn receive_wrong_sequence() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    f.send_up_hex("40 01 02"); // FIR=1, FIN=0, SEQ=0
    f.send_up_hex("82 03 04"); // FIR=0, FIN=1, SEQ=2 (expected 1)
    assert_eq!(f.transport.rx.statistics.num_transport_ignore, 1);
    assert!(f.received_messages().is_empty());
}

#[test]
fn packets_can_be_of_varying_size() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    f.send_up_hex("40 0A 0B 0C");
    f.send_up_hex("81 0D 0E 0F");
    assert_eq!(f.received_messages(), vec!["0A 0B 0C 0D 0E 0F"]);
}

#[test]
fn receive_single_packet() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    f.send_up_hex("C0 77");
    assert_eq!(f.received_messages(), vec!["77"]);
}

#[test]
fn receive_largest_possible_apdu() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    let full_apdu: Vec<u8> = (0..DEFAULT_MAX_APDU_SIZE)
        .map(|i| (i & 0xFF) as u8)
        .collect();

    let mut tx = TransportTx::new(Message::new(Addresses::new(1024, 1), full_apdu.clone()));
    let mut segments = Vec::new();
    while tx.has_value() {
        segments.push(tx.segment().to_vec());
        tx.advance();
    }
    assert_eq!(
        segments.len(),
        DEFAULT_MAX_APDU_SIZE.div_ceil(MAX_TPDU_PAYLOAD)
    );

    for seg in segments {
        f.transport
            .handle_link_event(&LinkEvent::Message(Message::new(
                Addresses::new(1024, 1),
                seg,
            )));
        f.events.extend(f.transport.drain_events());
    }

    assert_eq!(f.received_messages(), vec![to_hex(&full_apdu)]);
}

#[test]
fn receive_buffer_overflow() {
    let mut f = TransportFixture::new(true, 4);
    f.send_up_hex("40 11 22 33");
    assert_eq!(f.transport.rx.statistics.num_transport_rx, 1);
    assert_eq!(f.transport.rx.statistics.num_transport_buffer_overflow, 0);

    f.send_up_hex("81 44 55");
    assert_eq!(f.transport.rx.statistics.num_transport_rx, 2);
    assert_eq!(f.transport.rx.statistics.num_transport_buffer_overflow, 1);
    assert!(f.received_messages().is_empty());
}

#[test]
fn receive_new_fir_discards_previous() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    f.send_up_hex("40 01 02 03");
    assert!(f.received_messages().is_empty());

    f.send_up_hex("C0 AB CD");
    assert_eq!(f.received_messages(), vec!["AB CD"]);
    assert_eq!(f.transport.rx.statistics.num_transport_discard, 1);
}

#[test]
fn send_sequence_increments_across_transmissions() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);

    assert!(f.send_down_hex("11"));
    // Reject concurrent send while sending
    assert!(!f.send_down_hex("22"));

    let frame1 = f.link.pop_transmit().expect("frame1");
    // Unconfirmed user data frame payload starts at byte 10; first byte is TPDU header
    assert_eq!(frame1[10], TransportHeader::to_byte(true, true, 0));
    assert_eq!(frame1[11], 0x11);

    f.link.on_tx_ready();
    f.pump_link_events();
    assert!(!f.transport.is_sending());

    assert!(f.send_down_hex("22"));
    let frame2 = f.link.pop_transmit().expect("frame2");
    assert_eq!(frame2[10], TransportHeader::to_byte(true, true, 1));
    assert_eq!(frame2[11], 0x22);
}

#[test]
fn closed_while_sending() {
    let mut f = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    assert!(f.send_down_hex("11"));
    f.lower_layer_down();
    assert!(!f.transport.is_online());
    assert!(!f.transport.is_sending());
}
