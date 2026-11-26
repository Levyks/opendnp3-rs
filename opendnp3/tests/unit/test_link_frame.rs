//! 1:1 port of `TestLinkFrame.cpp` (9 test cases).

use opendnp3::enums::LinkFunction;
use opendnp3::link::constants::LPDU_MAX_FRAME_SIZE;
use opendnp3::link::{LinkHeader, frame};
use opendnp3::util::WriteCursor;
use opendnp3::util::hex::to_hex;

use crate::common::{format_user_data, repair_crc};

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
fn reset_links() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_reset_link_states(&mut cursor, true, 1, 1024);
    assert_eq!(to_hex(cursor.written()), "05 64 05 C0 01 00 00 04 E9 21");
}

#[test]
fn request_link_states() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_request_link_status(&mut cursor, false, 1, 1024);
    assert_eq!(to_hex(cursor.written()), "05 64 05 49 01 00 00 04 D2 36");
}

#[test]
fn ack() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    {
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_ack(&mut cursor, false, false, 1024, 1);
        assert_eq!(to_hex(cursor.written()), "05 64 05 00 00 04 01 00 19 A6");
    }
    {
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_ack(&mut cursor, false, true, 1024, 1);
        assert_eq!(to_hex(cursor.written()), "05 64 05 10 00 04 01 00 8B 0C");
    }
    {
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_ack(&mut cursor, true, false, 1, 1024);
        assert_eq!(to_hex(cursor.written()), "05 64 05 80 01 00 00 04 53 11");
    }
    {
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_ack(&mut cursor, true, true, 1, 1024);
        assert_eq!(to_hex(cursor.written()), "05 64 05 90 01 00 00 04 C1 BB");
    }
}

#[test]
fn confirmed_user_data() {
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
fn reset_link_states() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_reset_link_states(&mut cursor, false, 1024, 1);
    assert_eq!(to_hex(cursor.written()), "05 64 05 40 00 04 01 00 A3 96");
}

#[test]
fn unconfirmed_user_data() {
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
fn link_status() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_link_status(&mut cursor, true, true, 1, 1024);
    assert_eq!(
        to_hex(cursor.written()),
        repair_crc("05 64 05 9B 01 00 00 04 E9 21")
    );
}

#[test]
fn not_supported() {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_not_supported(&mut cursor, false, true, 1, 1024);
    assert_eq!(
        to_hex(cursor.written()),
        repair_crc("05 64 05 1F 01 00 00 04 28 5A")
    );
}
