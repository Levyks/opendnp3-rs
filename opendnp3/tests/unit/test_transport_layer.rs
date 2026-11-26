//! 1:1 port of `TestTransportLayer.cpp` (15 test cases).

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
    upper_online: bool,
    received: Vec<u8>,
    tx_ready_count: usize,
    link_sends: Vec<String>,
}

impl TransportFixture {
    fn new(start_online: bool, max_rx_frag_size: usize) -> Self {
        let mut f = Self {
            clock: ManualClock::new(),
            link: LinkLayer::new(LinkConfig::new(true)),
            transport: TransportLayer::new(max_rx_frag_size),
            upper_online: false,
            received: Vec::new(),
            tx_ready_count: 0,
            link_sends: Vec::new(),
        };
        if start_online {
            f.on_lower_layer_up();
        }
        f
    }

    fn drain(&mut self) {
        for ev in self.link.drain_events() {
            self.transport.handle_link_event(&ev);
        }
        for ev in self.transport.drain_events() {
            match ev {
                TransportEvent::LowerLayerUp => self.upper_online = true,
                TransportEvent::LowerLayerDown => self.upper_online = false,
                TransportEvent::Message(msg) => self.received.extend_from_slice(&msg.payload),
                TransportEvent::TxReady => self.tx_ready_count += 1,
            }
        }
    }

    fn on_lower_layer_up(&mut self) -> bool {
        let ok = self.link.on_lower_layer_up(self.clock.now());
        self.drain();
        ok
    }

    fn on_lower_layer_down(&mut self) -> bool {
        let ok = self.link.on_lower_layer_down();
        self.drain();
        ok
    }

    fn on_tx_ready(&mut self) -> bool {
        let ok = self.link.on_tx_ready();
        self.drain();
        ok
    }

    fn send_up(&mut self, hex: &str) {
        let bytes = from_hex(hex).unwrap();
        self.transport
            .handle_link_event(&LinkEvent::Message(Message::new(
                Addresses::new(1024, 1),
                bytes,
            )));
        self.drain();
    }

    fn send_down(&mut self, hex: &str) -> bool {
        let bytes = from_hex(hex).unwrap();
        let ok = self.transport.begin_transmit(
            Message::new(Addresses::new(1, 1024), bytes),
            &mut self.link,
            self.clock.now(),
        );
        while let Some(frame) = self.link.pop_transmit() {
            // Extract TPDU payload from the unconfirmed user data frame
            let mut parser = opendnp3::link::FrameParser::new();
            for parsed in parser.push_bytes(&frame) {
                self.link_sends.push(to_hex(&parsed.payload));
            }
        }
        self.drain();
        ok
    }

    fn pop_write(&mut self) -> String {
        self.link_sends.remove(0)
    }

    fn get_data(hdr_hex: &str) -> String {
        let mut s = hdr_hex.to_string();
        for _ in 0..MAX_TPDU_PAYLOAD {
            s.push_str(" 00");
        }
        s
    }

    fn generate_packet_sequence(
        num_packets: usize,
        last_packet_len: usize,
    ) -> (String, Vec<String>) {
        let mut packets = Vec::with_capacity(num_packets);
        let mut full_apdu = Vec::new();
        for i in 0..num_packets {
            let fir = i == 0;
            let fin = i == num_packets - 1;
            let seq = (i % 64) as u8;
            let hdr = TransportHeader::to_byte(fir, fin, seq);
            let len = if fin {
                last_packet_len
            } else {
                MAX_TPDU_PAYLOAD
            };
            let mut pkt = Vec::with_capacity(1 + len);
            pkt.push(hdr);
            for j in 0..len {
                let b = ((i + j) & 0xFF) as u8;
                pkt.push(b);
                full_apdu.push(b);
            }
            packets.push(to_hex(&pkt));
        }
        (to_hex(&full_apdu), packets)
    }
}

#[test]
fn repeat_sends_do_not_log_or_change_statistics() {
    let hs = from_hex("12 34 56").unwrap();
    let mut tx = TransportTx::new(Message::new(Addresses::default(), hs));

    let seg1 = tx.segment().to_vec();
    assert_eq!(to_hex(&seg1), "C0 12 34 56");
    assert_eq!(tx.statistics.num_transport_tx, 1);

    let seg2 = tx.segment().to_vec();
    assert_eq!(to_hex(&seg2), "C0 12 34 56");
    assert_eq!(tx.statistics.num_transport_tx, 1);
}

#[test]
fn state_offline() {
    let mut test = TransportFixture::new(false, DEFAULT_MAX_APDU_SIZE);
    assert!(!test.send_down("00"));
    assert!(!test.on_tx_ready());
    assert!(!test.on_lower_layer_down());
}

#[test]
fn state_ready() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    assert!(test.upper_online);
    test.on_lower_layer_down();
    assert!(!test.upper_online);
    test.on_lower_layer_up();
    assert!(test.upper_online);

    assert!(!test.on_lower_layer_up());
    assert!(!test.on_tx_ready());
}

#[test]
fn allows_header_only_final_frame() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_up("41 DE AD BE EF");
    test.send_up("82");
    assert_eq!(to_hex(&test.received), "DE AD BE EF");
}

#[test]
fn receive_no_fir() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_up("80 77");
    assert_eq!(test.transport.rx.statistics.num_transport_ignore, 1);
}

#[test]
fn receive_wrong_sequence() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_up(&TransportFixture::get_data("40"));
    test.send_up(&TransportFixture::get_data("82"));
    assert_eq!(test.transport.rx.statistics.num_transport_ignore, 1);
}

#[test]
fn packets_can_be_of_varying_size() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_up("40 0A 0B 0C");
    test.send_up("81 0D 0E 0F");
    assert_eq!(to_hex(&test.received), "0A 0B 0C 0D 0E 0F");
}

#[test]
fn receive_single_packet() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_up("C0 77");
    assert_eq!(to_hex(&test.received), "77");
}

#[test]
fn receive_largest_possible_apdu() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    let num_packets = DEFAULT_MAX_APDU_SIZE.div_ceil(MAX_TPDU_PAYLOAD);
    let last_packet_len = DEFAULT_MAX_APDU_SIZE % MAX_TPDU_PAYLOAD;
    let last_packet_len = if last_packet_len == 0 {
        MAX_TPDU_PAYLOAD
    } else {
        last_packet_len
    };

    let (apdu, packets) = TransportFixture::generate_packet_sequence(num_packets, last_packet_len);
    for pkt in &packets {
        test.send_up(pkt);
    }
    assert_eq!(to_hex(&test.received), apdu);
}

#[test]
fn receive_buffer_overflow() {
    let mut test = TransportFixture::new(true, 4);
    test.send_up("40 11 22 33");
    {
        let stats = test.transport.rx.statistics;
        assert_eq!(stats.num_transport_rx, 1);
        assert_eq!(stats.num_transport_buffer_overflow, 0);
    }
    test.send_up("81 44 55");
    {
        let stats = test.transport.rx.statistics;
        assert_eq!(stats.num_transport_rx, 2);
        assert_eq!(stats.num_transport_buffer_overflow, 1);
    }
}

#[test]
fn receive_new_fir() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_up(&TransportFixture::get_data("40"));
    assert!(test.received.is_empty());

    test.send_up("C0 AB CD");
    assert_eq!(to_hex(&test.received), "AB CD");
    assert_eq!(test.transport.rx.statistics.num_transport_discard, 1);
}

#[test]
fn state_sending() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    assert!(test.send_down("11"));
    assert_eq!(test.pop_write(), "C0 11");

    assert!(!test.send_down("00"));
    assert!(!test.on_lower_layer_up());

    test.send_up("C0 77");
    assert_eq!(to_hex(&test.received), "77");

    test.on_tx_ready();
    assert_eq!(test.tx_ready_count, 1);
    assert!(!test.on_tx_ready());
}

#[test]
fn send_success() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_down("11");
    assert_eq!(test.pop_write(), "C0 11");
    test.on_tx_ready();

    test.send_down("11");
    assert_eq!(test.pop_write(), "C1 11");
    test.on_tx_ready();
    assert_eq!(test.tx_ready_count, 2);
}

#[test]
fn closed_while_sending() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    test.send_down("11");
    test.on_lower_layer_down();
    assert!(!test.upper_online);
}

#[test]
fn send_full_apdu() {
    let mut test = TransportFixture::new(true, DEFAULT_MAX_APDU_SIZE);
    let num_packets = DEFAULT_MAX_APDU_SIZE.div_ceil(MAX_TPDU_PAYLOAD);
    let last_packet_len = DEFAULT_MAX_APDU_SIZE % MAX_TPDU_PAYLOAD;
    let last_packet_len = if last_packet_len == 0 {
        MAX_TPDU_PAYLOAD
    } else {
        last_packet_len
    };

    let (apdu, packets) = TransportFixture::generate_packet_sequence(num_packets, last_packet_len);
    test.send_down(&apdu);

    // LinkLayer transmits one segment per on_tx_ready cycle
    let mut sent = Vec::new();
    while !test.link_sends.is_empty() {
        sent.push(test.pop_write());
        test.on_tx_ready();
        while let Some(frame) = test.link.pop_transmit() {
            let mut parser = opendnp3::link::FrameParser::new();
            for parsed in parser.push_bytes(&frame) {
                test.link_sends.push(to_hex(&parsed.payload));
            }
        }
    }

    assert_eq!(num_packets, sent.len());
    assert_eq!(packets, sent);
}
