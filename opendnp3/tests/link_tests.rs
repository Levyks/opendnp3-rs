//! Ported unit tests for the DNP3 link layer:
//! - `TestCRC.cpp`
//! - `TestLinkAddresses.cpp`
//! - `TestLinkFrame.cpp`
//! - `TestLinkReceiver.cpp`
//! - `TestLinkLayer.cpp`
//! - `TestLinkLayerKeepAlive.cpp`

use std::time::Duration;

use opendnp3::clock::ManualClock;
use opendnp3::enums::LinkFunction;
use opendnp3::link::constants::{LPDU_DATA_BLOCK_SIZE, LPDU_HEADER_SIZE, LPDU_MAX_FRAME_SIZE};
use opendnp3::link::{
    Addresses, FrameParser, LinkBroadcastAddress, LinkConfig, LinkEvent, LinkHeader,
    LinkHeaderFields, LinkLayer, TransportSegment, crc, frame,
};
use opendnp3::util::WriteCursor;
use opendnp3::util::hex::{from_hex, to_hex};

/// Repair header and body CRCs in a hex-encoded FT3 frame (mirrors C++ `RepairCRC`).
fn repair_crc(hex: &str) -> String {
    let mut bytes = from_hex(hex).expect("valid hex");
    assert!(bytes.len() >= LPDU_HEADER_SIZE);
    crc::add_crc(&mut bytes, 8);
    let mut offset = LPDU_HEADER_SIZE;
    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        assert!(remaining >= 2);
        let block_len = (remaining - 2).min(LPDU_DATA_BLOCK_SIZE);
        crc::add_crc(&mut bytes[offset..], block_len);
        offset += block_len + 2;
    }
    to_hex(&bytes)
}

fn format_user_data(
    is_master: bool,
    is_confirmed: bool,
    dest: u16,
    src: u16,
    data_hex: &str,
    fcb: bool,
) -> String {
    let data = from_hex(data_hex).expect("valid hex");
    assert!(data.len() <= 250);
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    if is_confirmed {
        frame::format_confirmed_user_data(&mut cursor, is_master, fcb, dest, src, &data).unwrap();
    } else {
        frame::format_unconfirmed_user_data(&mut cursor, is_master, dest, src, &data).unwrap();
    }
    to_hex(cursor.written())
}

fn link_hex_ack(is_master: bool, is_rcv_buff_full: bool, dest: u16, src: u16) -> String {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, is_master, is_rcv_buff_full, dest, src);
    to_hex(cursor.written())
}

fn link_hex_link_status(is_master: bool, is_rcv_buff_full: bool, dest: u16, src: u16) -> String {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_link_status(&mut cursor, is_master, is_rcv_buff_full, dest, src);
    to_hex(cursor.written())
}

fn link_hex_request_link_status(is_master: bool, dest: u16, src: u16) -> String {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_request_link_status(&mut cursor, is_master, dest, src);
    to_hex(cursor.written())
}

// =========================================================================
// TestCRC.cpp
// =========================================================================

#[test]
fn crc_test() {
    let hs = from_hex("05 64 05 C0 01 00 00 04 E9 21").unwrap();
    assert_eq!(hs.len(), 10);
    assert_eq!(crc::calc_crc(&hs[..8]), 0x21E9);
}

// =========================================================================
// TestLinkAddresses.cpp
// =========================================================================

#[test]
fn link_addresses_equality_comparison() {
    let lr1 = Addresses::new(1, 2);
    let lr2 = Addresses::new(1, 3);
    let lr3 = Addresses::new(1, 3);

    assert_ne!(lr1, lr2);
    assert_eq!(lr3, lr2);
}

#[test]
fn link_addresses_is_broadcast() {
    assert!(Addresses::new(1, 0xFFFD).is_broadcast());
    assert!(Addresses::new(1, 0xFFFE).is_broadcast());
    assert!(Addresses::new(1, 0xFFFF).is_broadcast());
    assert!(!Addresses::new(1, 1024).is_broadcast());
}

// =========================================================================
// TestLinkFrame.cpp
// =========================================================================

#[test]
fn link_header_change_fcb() {
    let mut hdr = LinkHeader::new(
        5,
        1,
        1024,
        true,
        true,
        true,
        LinkFunction::PriConfirmedUserData,
    );
    assert_eq!(
        LinkHeader::control_byte(true, true, true, LinkFunction::PriConfirmedUserData),
        hdr.control()
    );
    hdr.change_fcb(false);
    assert_eq!(
        LinkHeader::control_byte(true, false, true, LinkFunction::PriConfirmedUserData),
        hdr.control()
    );
}

#[test]
fn frame_reset_links() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_reset_link_states(&mut cursor, true, 1, 1024);
    assert_eq!(to_hex(cursor.written()), "05 64 05 C0 01 00 00 04 E9 21");
}

#[test]
fn frame_request_link_states() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_request_link_status(&mut cursor, false, 1, 1024);
    assert_eq!(to_hex(cursor.written()), "05 64 05 49 01 00 00 04 D2 36");
}

#[test]
fn frame_ack() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];

    // ACK - Outstation (DFC = false)
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, false, false, 1024, 1);
    assert_eq!(to_hex(cursor.written()), "05 64 05 00 00 04 01 00 19 A6");

    // ACK - Outstation (DFC = true)
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, false, true, 1024, 1);
    assert_eq!(to_hex(cursor.written()), "05 64 05 10 00 04 01 00 8B 0C");

    // ACK - Master (DFC = false)
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, true, false, 1, 1024);
    assert_eq!(to_hex(cursor.written()), "05 64 05 80 01 00 00 04 53 11");

    // ACK - Master (DFC = true)
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, true, true, 1, 1024);
    assert_eq!(to_hex(cursor.written()), "05 64 05 90 01 00 00 04 C1 BB");
}

#[test]
fn frame_confirmed_user_data() {
    let hex = format_user_data(
        true,
        true,
        1,
        1024,
        "C0 C3 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06",
        true,
    );
    assert_eq!(
        hex,
        "05 64 14 F3 01 00 00 04 0A 3B C0 C3 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06 9A 12"
    );

    let hex2 = format_user_data(
        false,
        true,
        1024,
        1,
        "C1 E3 81 96 00 02 01 28 01 00 00 00 01 02 01 28 01 00 01 00 01 02 01 28 01 00 02 00 01 \
         02 01 28 01 00 03 00 01 20 02 28 01 00 00 00 01 00 00 20 02 28 01 00 01 00 01 00 00 01 \
         01 01 00 00 03 00 00 1E 02 01 00 00 01 00 01 00 00 01 00 00",
        true,
    );
    assert_eq!(
        hex2,
        "05 64 53 73 00 04 01 00 03 FC C1 E3 81 96 00 02 01 28 01 00 00 00 01 02 01 28 05 24 01 00 01 00 01 02 \
         01 28 01 00 02 00 01 02 01 28 B4 77 01 00 03 00 01 20 02 28 01 00 00 00 01 00 00 20 A5 25 02 28 01 00 \
         01 00 01 00 00 01 01 01 00 00 03 00 2F AC 00 1E 02 01 00 00 01 00 01 00 00 01 00 00 16 ED"
    );
}

#[test]
fn frame_reset_link_states() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_reset_link_states(&mut cursor, false, 1024, 1);
    assert_eq!(to_hex(cursor.written()), "05 64 05 40 00 04 01 00 A3 96");
}

#[test]
fn frame_unconfirmed_user_data() {
    let hex = format_user_data(
        false,
        false,
        1024,
        1,
        "C1 E3 81 96 00 02 01 28 01 00 00 00 01 02 01 28 01 00 01 00 01 02 01 28 01 00 02 00 01 \
         02 01 28 01 00 03 00 01 20 02 28 01 00 00 00 01 00 00 20 02 28 01 00 01 00 01 00 00 01 \
         01 01 00 00 03 00 00 1E 02 01 00 00 01 00 01 00 00 01 00 00",
        false,
    );
    assert_eq!(
        hex,
        repair_crc(
            "05 64 53 44 00 04 01 00 FF FF C1 E3 81 96 00 02 01 28 01 00 00 00 01 02 01 28 FF FF 01 00 01 00 01 \
             02 01 28 01 00 02 00 01 02 01 28 FF FF 01 00 03 00 01 20 02 28 01 00 00 00 01 00 00 20 FF FF 02 28 \
             01 00 01 00 01 00 00 01 01 01 00 00 03 00 FF FF 00 1E 02 01 00 00 01 00 01 00 00 01 00 00 FF FF"
        )
    );
}

#[test]
fn frame_link_status() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_link_status(&mut cursor, true, true, 1, 1024);
    assert_eq!(
        to_hex(cursor.written()),
        repair_crc("05 64 05 9B 01 00 00 04 E9 21")
    );
}

#[test]
fn frame_not_supported() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_not_supported(&mut cursor, false, true, 1, 1024);
    assert_eq!(
        to_hex(cursor.written()),
        repair_crc("05 64 05 1F 01 00 00 04 28 5A")
    );
}

// =========================================================================
// TestLinkReceiver.cpp
// =========================================================================

#[test]
fn parser_header_crc_error() {
    let mut parser = FrameParser::new();
    let frames = parser.push_bytes(&from_hex("05 64 05 C0 01 00 00 04 E9 20").unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_header_crc_error, 1);
}

#[test]
fn parser_body_crc_error() {
    let mut parser = FrameParser::new();
    let frames = parser.push_bytes(
        &from_hex(
            "05 64 14 F3 01 00 00 04 0A 3B C0 C3 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06 9A 11",
        )
        .unwrap(),
    );
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_body_crc_error, 1);
}

#[test]
fn parser_bad_length_error() {
    let mut parser = FrameParser::new();
    let frames =
        parser.push_bytes(&from_hex(&repair_crc("05 64 01 C0 01 00 00 04 E9 21")).unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_bad_length, 1);
}

#[test]
fn parser_unexpected_data() {
    let mut parser = FrameParser::new();
    let frames =
        parser.push_bytes(&from_hex(&repair_crc("05 64 08 C0 01 00 00 04 E9 21")).unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_bad_length, 1);
}

#[test]
fn parser_absence_of_data() {
    let mut parser = FrameParser::new();
    let frames =
        parser.push_bytes(&from_hex(&repair_crc("05 64 05 73 00 04 01 00 03 FC")).unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_bad_length, 1);
}

#[test]
fn parser_unknown_function() {
    let mut parser = FrameParser::new();
    let frames =
        parser.push_bytes(&from_hex(&repair_crc("05 64 05 C6 01 00 00 04 E9 21")).unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_bad_function_code, 1);
}

#[test]
fn parser_unexpected_fcv() {
    let mut parser = FrameParser::new();
    let frames =
        parser.push_bytes(&from_hex(&repair_crc("05 64 05 D0 01 00 00 04 E9 21")).unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_bad_fcv, 1);
}

#[test]
fn parser_unexpected_fcb() {
    let mut parser = FrameParser::new();
    let frames =
        parser.push_bytes(&from_hex(&repair_crc("05 64 05 20 00 04 01 00 19 A6")).unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_bad_fcb, 1);
}

#[test]
fn parser_combined_failures() {
    let mut parser = FrameParser::new();
    let combined = format!(
        "{} {}",
        repair_crc("05 64 05 20 00 04 01 00 19 A6"),
        repair_crc("05 64 05 D0 01 00 00 04 E9 21")
    );
    let frames = parser.push_bytes(&from_hex(&combined).unwrap());
    assert!(frames.is_empty());
    assert_eq!(parser.statistics.num_bad_fcv, 1);
    assert_eq!(parser.statistics.num_bad_fcb, 1);
}

#[test]
fn parser_read_sec_to_pri_frames() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut parser = FrameParser::new();

    // ACK
    let mut c = WriteCursor::new(&mut buf);
    frame::format_ack(&mut c, true, false, 1, 2);
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::SecAck);
    assert!(f[0].fields.is_from_master);
    assert!(!f[0].fields.fcvdfc);
    assert_eq!(f[0].fields.addresses, Addresses::new(2, 1));

    // NACK
    let mut c = WriteCursor::new(&mut buf);
    frame::format_nack(&mut c, false, true, 1, 2);
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::SecNack);
    assert!(!f[0].fields.is_from_master);
    assert!(f[0].fields.fcvdfc);

    // LinkStatus
    let mut c = WriteCursor::new(&mut buf);
    frame::format_link_status(&mut c, true, true, 1, 2);
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::SecLinkStatus);
    assert!(f[0].fields.fcvdfc);

    // NotSupported
    let mut c = WriteCursor::new(&mut buf);
    frame::format_not_supported(&mut c, true, false, 1, 2);
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::SecNotSupported);
    assert!(!f[0].fields.fcvdfc);
}

#[test]
fn parser_read_pri_to_sec_frames() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut parser = FrameParser::new();

    // TestLinkStates
    let mut c = WriteCursor::new(&mut buf);
    frame::format_test_link_status(&mut c, false, true, 1, 2);
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::PriTestLinkStates);
    assert!(f[0].fields.fcb);

    // ResetLinkStates
    let mut c = WriteCursor::new(&mut buf);
    frame::format_reset_link_states(&mut c, false, 1, 2);
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::PriResetLinkStates);

    // RequestLinkStatus
    let mut c = WriteCursor::new(&mut buf);
    frame::format_request_link_status(&mut c, true, 1, 2);
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::PriRequestLinkStatus);

    // UnconfirmedUserData (250 bytes)
    let data: Vec<u8> = (0u8..250).collect();
    let mut c = WriteCursor::new(&mut buf);
    frame::format_unconfirmed_user_data(&mut c, true, 1, 2, &data).unwrap();
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::PriUnconfirmedUserData);
    assert_eq!(f[0].payload, data);

    // ConfirmedUserData (250 bytes)
    let mut c = WriteCursor::new(&mut buf);
    frame::format_confirmed_user_data(&mut c, true, true, 1, 2, &data).unwrap();
    let f = parser.push_bytes(c.written());
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].fields.func, LinkFunction::PriConfirmedUserData);
    assert!(f[0].fields.fcb);
    assert_eq!(f[0].payload, data);
}

#[test]
fn parser_two_packets_and_resync_and_many_receives() {
    let mut parser = FrameParser::new();
    let frames = parser.push_bytes(
        &from_hex("05 64 05 C0 01 00 00 04 E9 21 05 64 05 C0 01 00 00 04 E9 21").unwrap(),
    );
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[1].fields.func, LinkFunction::PriResetLinkStates);
    assert_eq!(frames[1].fields.addresses, Addresses::new(1024, 1));

    // Resync0564
    let mut parser = FrameParser::new();
    let frames = parser.push_bytes(&from_hex("05 64 05 64 05 C0 01 00 00 04 E9 21").unwrap());
    assert_eq!(parser.statistics.num_header_crc_error, 1);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].fields.func, LinkFunction::PriResetLinkStates);

    // ManyReceives
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut c = WriteCursor::new(&mut buf);
    frame::format_ack(&mut c, true, false, 1, 2);
    let ack = c.written().to_vec();
    let mut parser = FrameParser::new();
    for _ in 1..100 {
        let frames = parser.push_bytes(&ack);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].fields.func, LinkFunction::SecAck);
    }
}

// =========================================================================
// TestLinkLayer.cpp & TestLinkLayerKeepAlive.cpp
// =========================================================================

struct MockSegment {
    addresses: Addresses,
    data: Vec<u8>,
    done: bool,
}

impl MockSegment {
    fn new(data: Vec<u8>, addresses: Addresses) -> Self {
        Self {
            addresses,
            data,
            done: false,
        }
    }
}

impl TransportSegment for MockSegment {
    fn addresses(&self) -> Addresses {
        self.addresses
    }
    fn has_value(&self) -> bool {
        !self.done
    }
    fn segment(&mut self) -> &[u8] {
        &self.data
    }
    fn advance(&mut self) -> bool {
        self.done = true;
        false
    }
}

struct LinkFixture {
    clock: ManualClock,
    link: LinkLayer,
    writes: Vec<Vec<u8>>,
    events: Vec<LinkEvent>,
}

impl LinkFixture {
    fn new(config: LinkConfig) -> Self {
        Self {
            clock: ManualClock::new(),
            link: LinkLayer::new(config),
            writes: Vec::new(),
            events: Vec::new(),
        }
    }

    fn default_master() -> Self {
        Self::new(LinkConfig::new(true))
    }

    fn collect(&mut self) {
        while let Some(frame) = self.link.pop_transmit() {
            self.writes.push(frame);
        }
        self.events.extend(self.link.drain_events());
    }

    fn on_lower_layer_up(&mut self) -> bool {
        let r = self.link.on_lower_layer_up(self.clock.now());
        self.collect();
        r
    }

    fn on_lower_layer_down(&mut self) -> bool {
        let r = self.link.on_lower_layer_down();
        self.collect();
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn on_frame(
        &mut self,
        func: LinkFunction,
        is_master: bool,
        fcb: bool,
        fcvdfc: bool,
        dest: u16,
        src: u16,
        userdata: &[u8],
    ) -> bool {
        let hdr = LinkHeaderFields {
            func,
            is_from_master: is_master,
            fcb,
            fcvdfc,
            addresses: Addresses::new(src, dest),
        };
        let r = self.link.on_frame(&hdr, userdata, self.clock.now());
        self.collect();
        r
    }

    fn on_tx_ready(&mut self) -> bool {
        let r = self.link.on_tx_ready();
        self.collect();
        r
    }

    fn advance(&mut self, dur: Duration) {
        self.clock.advance(dur);
        self.link.on_timeout(self.clock.now());
        self.collect();
    }

    fn advance_to_next_wakeup(&mut self) -> bool {
        if let Some(deadline) = self.link.next_wakeup() {
            let now = self.clock.now();
            if deadline > now {
                self.clock.advance(deadline - now);
            }
            self.link.on_timeout(self.clock.now());
            self.collect();
            true
        } else {
            false
        }
    }

    fn last_write_hex(&self) -> String {
        self.writes.last().map(|w| to_hex(w)).unwrap_or_default()
    }

    fn count_events(&self, pred: impl Fn(&LinkEvent) -> bool) -> usize {
        self.events.iter().filter(|e| pred(e)).count()
    }
}

#[test]
fn link_layer_closed_state() {
    let mut t = LinkFixture::default_master();
    let seg = Box::new(MockSegment::new(vec![0x00], Addresses::default()));
    assert!(!t.link.send(seg, t.clock.now()));
    assert!(!t.on_lower_layer_down());
    assert!(!t.on_frame(LinkFunction::SecAck, false, false, false, 1, 2, &[]));
}

#[test]
fn link_layer_forwards_on_lower_layer_up() {
    let mut t = LinkFixture::default_master();
    assert!(!t.link.is_online());
    assert!(t.on_lower_layer_up());
    assert!(t.link.is_online());
    assert!(t.events.contains(&LinkEvent::LowerLayerUp));
    assert!(!t.on_lower_layer_up());
}

#[test]
fn link_layer_validation_errors() {
    // ValidatesMasterOutstationBit
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, true, false, false, 1, 1024, &[]);
    assert_eq!(t.link.statistics.num_bad_master_bit, 1);

    // ValidatesSourceAddress
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1023, &[]);
    assert_eq!(t.link.statistics.num_unknown_source, 1);

    // ValidatesDestinationAddress
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, false, false, false, 2, 1024, &[]);
    assert_eq!(t.link.statistics.num_unknown_destination, 1);

    // SecToPriNoContext
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1024, &[]);
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);
}

#[test]
fn link_layer_unconfirmed_and_confirmed_user_data() {
    // UnconfirmedDataPassedUpFromIdleUnreset
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    let data: Vec<u8> = (0u8..250).collect();
    t.on_frame(
        LinkFunction::PriUnconfirmedUserData,
        false,
        false,
        false,
        1,
        1024,
        &data,
    );
    let msgs: Vec<_> = t
        .events
        .iter()
        .filter_map(|e| match e {
            LinkEvent::Message(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].payload, data);

    // UnconfirmedBroadcastDataPassedUpFromIdleUnreset
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriUnconfirmedUserData,
        false,
        false,
        false,
        LinkBroadcastAddress::ShallConfirm.to_u16(),
        1024,
        &data,
    );
    assert_eq!(t.count_events(|e| matches!(e, LinkEvent::Message(_))), 1);

    // ConfirmedDataIgnoredFromIdleUnreset
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    t.on_frame(
        LinkFunction::PriConfirmedUserData,
        false,
        false,
        false,
        1,
        1024,
        &data,
    );
    assert_eq!(t.count_events(|e| matches!(e, LinkEvent::Message(_))), 0);
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);
}

#[test]
fn link_layer_secondary_reset_and_confirmed_data() {
    let mut t = LinkFixture::default_master();
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
    assert_eq!(t.writes.len(), 1);
    assert_eq!(t.last_write_hex(), link_hex_ack(true, false, 1024, 1));
    t.on_tx_ready();

    // Confirmed user data with expected FCB=true
    let data: Vec<u8> = (0u8..250).collect();
    t.on_frame(
        LinkFunction::PriConfirmedUserData,
        false,
        true,
        false,
        1,
        1024,
        &data,
    );
    assert_eq!(t.writes.len(), 2);
    t.on_tx_ready();
    assert_eq!(t.count_events(|e| matches!(e, LinkEvent::Message(_))), 1);

    // Repeat with wrong FCB (still true instead of false) -> ACKed, no duplicate data pushed up
    t.events.clear();
    t.on_frame(
        LinkFunction::PriConfirmedUserData,
        false,
        true,
        false,
        1,
        1024,
        &data,
    );
    assert_eq!(t.writes.len(), 3);
    assert_eq!(t.count_events(|e| matches!(e, LinkEvent::Message(_))), 0);
}

#[test]
fn link_layer_broadcast_rejected_for_control_frames() {
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();
    let bcast = LinkBroadcastAddress::ShallConfirm.to_u16();
    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        bcast,
        1024,
        &[],
    );
    assert_eq!(t.writes.len(), 0);
    assert_eq!(t.link.statistics.num_unexpected_frame, 1);

    t.on_frame(
        LinkFunction::PriRequestLinkStatus,
        false,
        false,
        false,
        bcast,
        1024,
        &[],
    );
    assert_eq!(t.writes.len(), 0);
    assert_eq!(t.link.statistics.num_unexpected_frame, 2);

    t.on_frame(
        LinkFunction::PriTestLinkStates,
        false,
        false,
        false,
        bcast,
        1024,
        &[],
    );
    assert_eq!(t.writes.len(), 0);
    assert_eq!(t.link.statistics.num_unexpected_frame, 3);
}

#[test]
fn link_layer_request_status_and_test_link_states() {
    let mut t = LinkFixture::default_master();
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
    assert_eq!(t.writes.len(), 1);
    assert_eq!(
        t.last_write_hex(),
        link_hex_link_status(true, false, 1024, 1)
    );
    t.on_tx_ready();

    // TestLinkStates before reset -> ignored
    t.on_frame(
        LinkFunction::PriTestLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.writes.len(), 1);

    // Reset then TestLinkStates with FCB=true -> ACK
    t.on_frame(
        LinkFunction::PriResetLinkStates,
        false,
        false,
        false,
        1,
        1024,
        &[],
    );
    assert_eq!(t.writes.len(), 2);
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
    assert_eq!(t.writes.len(), 3);
    assert_eq!(t.last_write_hex(), link_hex_ack(true, false, 1024, 1));
}

#[test]
fn link_layer_send_unconfirmed_and_close_behavior() {
    let mut t = LinkFixture::default_master();
    t.on_lower_layer_up();

    let data: Vec<u8> = (0u8..250).collect();
    assert!(t.link.send(
        Box::new(MockSegment::new(data.clone(), Addresses::new(1, 1024))),
        t.clock.now()
    ));
    t.collect();
    assert_eq!(t.writes.len(), 1);
    t.on_tx_ready();
    assert_eq!(t.count_events(|e| *e == LinkEvent::TxReady), 1);

    // Close and reopen
    assert!(t.on_lower_layer_down());
    assert!(!t.link.is_online());
    assert!(t.on_lower_layer_up());
    assert!(t.link.send(
        Box::new(MockSegment::new(data, Addresses::new(1, 1024))),
        t.clock.now()
    ));
    t.collect();
    assert_eq!(t.writes.len(), 2);
}

#[test]
fn link_layer_keep_alive_flow() {
    let mut cfg = LinkConfig::new(true);
    cfg.keep_alive_timeout = Duration::from_secs(5);
    cfg.timeout = Duration::from_secs(1);

    // Keep-alive success
    let mut t = LinkFixture::new(cfg.clone());
    assert!(t.link.next_wakeup().is_none());
    t.on_lower_layer_up();
    assert!(t.link.next_wakeup().is_some());

    assert!(t.advance_to_next_wakeup());
    assert_eq!(t.count_events(|e| *e == LinkEvent::KeepAliveInitiated), 1);
    assert_eq!(
        t.last_write_hex(),
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
    assert_eq!(t.count_events(|e| *e == LinkEvent::KeepAliveSuccess), 1);
    assert_eq!(t.count_events(|e| *e == LinkEvent::KeepAliveFailure), 0);

    // Keep-alive failure on timeout
    let mut t = LinkFixture::new(cfg.clone());
    t.on_lower_layer_up();
    assert!(t.advance_to_next_wakeup());
    t.on_tx_ready();
    t.advance(cfg.timeout);
    assert_eq!(t.count_events(|e| *e == LinkEvent::KeepAliveFailure), 1);

    // Keep-alive stops on ACK without reporting failure
    let mut t = LinkFixture::new(cfg.clone());
    t.on_lower_layer_up();
    assert!(t.advance_to_next_wakeup());
    t.on_tx_ready();
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1024, &[]);
    assert_eq!(t.count_events(|e| *e == LinkEvent::KeepAliveFailure), 0);

    // Keep-alive timer restarts when frame is received
    let mut t = LinkFixture::new(cfg.clone());
    t.on_lower_layer_up();
    t.advance(Duration::from_secs(2));
    t.on_frame(LinkFunction::SecAck, false, false, false, 1, 1024, &[]);
    t.advance(Duration::from_secs(3));
    assert!(t.writes.is_empty());
    t.advance(Duration::from_secs(2));
    assert_eq!(
        t.last_write_hex(),
        link_hex_request_link_status(true, 1024, 1)
    );

    // Keep-alive disabled
    let mut cfg_disabled = LinkConfig::new(true);
    cfg_disabled.keep_alive_timeout = Duration::MAX;
    let mut t = LinkFixture::new(cfg_disabled);
    t.on_lower_layer_up();
    assert!(t.link.next_wakeup().is_none());
}
