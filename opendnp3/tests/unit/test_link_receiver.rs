//! 1:1 port of `TestLinkReceiver.cpp` (22 test cases).

use opendnp3::enums::LinkFunction;
use opendnp3::link::constants::LPDU_MAX_FRAME_SIZE;
use opendnp3::link::{FrameParser, LinkHeaderFields, frame};
use opendnp3::util::WriteCursor;
use opendnp3::util::hex::from_hex;

use crate::common::{incrementing_bytes, repair_crc};

struct LinkParserFixture {
    parser: FrameParser,
    frames: Vec<(LinkHeaderFields, Vec<u8>)>,
}

impl LinkParserFixture {
    fn new() -> Self {
        Self {
            parser: FrameParser::new(),
            frames: Vec::new(),
        }
    }

    fn write_bytes(&mut self, data: &[u8]) {
        for frame in self.parser.push_bytes(data) {
            self.frames.push((frame.fields, frame.payload));
        }
    }

    fn write_hex(&mut self, hex: &str) {
        let bytes = from_hex(hex).expect("valid hex");
        self.write_bytes(&bytes);
    }

    fn num_frames(&self) -> usize {
        self.frames.len()
    }

    fn check_last(&self, func: LinkFunction, is_master: bool, dest: u16, src: u16) -> bool {
        let Some((hdr, _)) = self.frames.last() else {
            return false;
        };
        hdr.func == func
            && hdr.is_from_master == is_master
            && hdr.addresses.destination == dest
            && hdr.addresses.source == src
    }

    fn check_last_with_dfc(
        &self,
        func: LinkFunction,
        is_master: bool,
        dfc: bool,
        dest: u16,
        src: u16,
    ) -> bool {
        self.check_last(func, is_master, dest, src) && self.frames.last().unwrap().0.fcvdfc == dfc
    }

    fn check_last_with_fcb(
        &self,
        func: LinkFunction,
        is_master: bool,
        fcb: bool,
        dest: u16,
        src: u16,
    ) -> bool {
        self.check_last(func, is_master, dest, src) && self.frames.last().unwrap().0.fcb == fcb
    }

    fn last_body(&self) -> &[u8] {
        &self.frames.last().unwrap().1
    }
}

#[test]
fn initialization_state() {
    let t = LinkParserFixture::new();
    assert_eq!(t.num_frames(), 0);
}

#[test]
fn header_crc_error() {
    let mut t = LinkParserFixture::new();
    t.write_hex("05 64 05 C0 01 00 00 04 E9 20");
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_header_crc_error, 1);
}

#[test]
fn body_crc_error() {
    let mut t = LinkParserFixture::new();
    t.write_hex("05 64 14 F3 01 00 00 04 0A 3B C0 C3 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06 9A 11");
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_body_crc_error, 1);
}

#[test]
fn bad_length_error() {
    let mut t = LinkParserFixture::new();
    t.write_hex(&repair_crc("05 64 01 C0 01 00 00 04 E9 21"));
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_bad_length, 1);
}

#[test]
fn unexpected_data() {
    let mut t = LinkParserFixture::new();
    t.write_hex(&repair_crc("05 64 08 C0 01 00 00 04 E9 21"));
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_bad_length, 1);
}

#[test]
fn absence_of_data() {
    let mut t = LinkParserFixture::new();
    t.write_hex(&repair_crc("05 64 05 73 00 04 01 00 03 FC"));
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_bad_length, 1);
}

#[test]
fn unknown_function() {
    let mut t = LinkParserFixture::new();
    t.write_hex(&repair_crc("05 64 05 C6 01 00 00 04 E9 21"));
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_bad_function_code, 1);
}

#[test]
fn unexpected_fcv() {
    let mut t = LinkParserFixture::new();
    t.write_hex(&repair_crc("05 64 05 D0 01 00 00 04 E9 21"));
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_bad_fcv, 1);
}

#[test]
fn unexpected_fcb() {
    let mut t = LinkParserFixture::new();
    t.write_hex(&repair_crc("05 64 05 20 00 04 01 00 19 A6"));
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_bad_fcb, 1);
}

#[test]
fn combined_failures() {
    let mut t = LinkParserFixture::new();
    let combined = format!(
        "{} {}",
        repair_crc("05 64 05 20 00 04 01 00 19 A6"),
        repair_crc("05 64 05 D0 01 00 00 04 E9 21")
    );
    t.write_hex(&combined);
    assert_eq!(t.num_frames(), 0);
    assert_eq!(t.parser.statistics.num_bad_fcv, 1);
    assert_eq!(t.parser.statistics.num_bad_fcb, 1);
}

#[test]
fn read_ack() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, true, false, 1, 2);

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last_with_dfc(LinkFunction::SecAck, true, false, 1, 2));
}

#[test]
fn read_nack() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_nack(&mut cursor, false, true, 1, 2);

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last_with_dfc(LinkFunction::SecNack, false, true, 1, 2));
}

#[test]
fn link_status() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_link_status(&mut cursor, true, true, 1, 2);

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last_with_dfc(LinkFunction::SecLinkStatus, true, true, 1, 2));
}

#[test]
fn not_supported() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_not_supported(&mut cursor, true, false, 1, 2);

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last_with_dfc(LinkFunction::SecNotSupported, true, false, 1, 2));
}

#[test]
fn test_link_states() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_test_link_status(&mut cursor, false, true, 1, 2);

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last_with_fcb(LinkFunction::PriTestLinkStates, false, true, 1, 2));
}

#[test]
fn reset_link_states() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_reset_link_states(&mut cursor, false, 1, 2);

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last(LinkFunction::PriResetLinkStates, false, 1, 2));
}

#[test]
fn request_link_status() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_request_link_status(&mut cursor, true, 1, 2);

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last(LinkFunction::PriRequestLinkStatus, true, 1, 2));
}

#[test]
fn unconfirmed_user_data() {
    let data = incrementing_bytes(250, 0);
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_unconfirmed_user_data(&mut cursor, true, 1, 2, &data).unwrap();

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last(LinkFunction::PriUnconfirmedUserData, true, 1, 2));
    assert_eq!(t.last_body(), data.as_slice());
}

#[test]
fn confirmed_user_data() {
    let data = incrementing_bytes(250, 0);
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_confirmed_user_data(&mut cursor, true, true, 1, 2, &data).unwrap();

    let mut t = LinkParserFixture::new();
    t.write_bytes(cursor.written());
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last_with_fcb(LinkFunction::PriConfirmedUserData, true, true, 1, 2));
    assert_eq!(t.last_body(), data.as_slice());
}

#[test]
fn test_two_packets() {
    let mut t = LinkParserFixture::new();
    t.write_hex("05 64 05 C0 01 00 00 04 E9 21 05 64 05 C0 01 00 00 04 E9 21");
    assert_eq!(t.num_frames(), 2);
    assert!(t.check_last(LinkFunction::PriResetLinkStates, true, 1, 1024));
}

#[test]
fn resync_0564() {
    let mut t = LinkParserFixture::new();
    t.write_hex("05 64 05 64 05 C0 01 00 00 04 E9 21");
    assert_eq!(t.parser.statistics.num_header_crc_error, 1);
    assert_eq!(t.num_frames(), 1);
    assert!(t.check_last(LinkFunction::PriResetLinkStates, true, 1, 1024));
}

#[test]
fn many_receives() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, true, false, 1, 2);
    let frame = cursor.written().to_vec();

    let mut t = LinkParserFixture::new();
    for i in 1..100 {
        t.write_bytes(&frame);
        assert_eq!(t.num_frames(), i);
        assert!(t.check_last_with_dfc(LinkFunction::SecAck, true, false, 1, 2));
    }
}
