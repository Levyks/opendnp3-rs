//! Frame (LPDU) formatting and body parsing.

use super::constants::*;
use super::crc;
use super::header::LinkHeader;
use crate::enums::LinkFunction;
use crate::util::WriteCursor;

/// Total on-wire size of a frame carrying `data_length` user bytes.
pub fn calc_frame_size(data_length: usize) -> usize {
    LPDU_HEADER_SIZE + calc_user_data_size(data_length)
}

/// On-wire size of the user-data section (data + interleaved block CRCs).
pub fn calc_user_data_size(data_length: usize) -> usize {
    if data_length == 0 {
        return 0;
    }
    let mod16 = data_length % LPDU_DATA_BLOCK_SIZE;
    let complete = (data_length / LPDU_DATA_BLOCK_SIZE) * LPDU_DATA_PLUS_CRC_SIZE;
    if mod16 > 0 {
        complete + mod16 + LPDU_CRC_SIZE
    } else {
        complete
    }
}

/// Strip block CRCs from received body octets into `dest`.
/// `src` is the on-wire body (data+CRC blocks); `len` is the user-data length.
pub fn read_user_data(src: &[u8], dest: &mut [u8]) {
    let mut read = 0usize;
    let mut written = 0usize;
    let mut remaining = dest.len();
    while remaining > 0 {
        let num = remaining.min(LPDU_DATA_BLOCK_SIZE);
        dest[written..written + num].copy_from_slice(&src[read..read + num]);
        read += num + LPDU_CRC_SIZE;
        written += num;
        remaining -= num;
    }
}

/// Validate all block CRCs in an on-wire body of `length` user bytes.
pub fn validate_body_crc(body: &[u8], length: usize) -> bool {
    let mut offset = 0usize;
    let mut remaining = length;
    while remaining > 0 {
        let num = remaining.min(LPDU_DATA_BLOCK_SIZE);
        if !crc::is_correct_crc(&body[offset..], num) {
            return false;
        }
        offset += num + LPDU_CRC_SIZE;
        remaining -= num;
    }
    true
}

/// Write user data with interleaved block CRCs.
fn write_user_data(src: &[u8], dest: &mut [u8]) {
    let mut read = 0usize;
    let mut written = 0usize;
    let mut remaining = src.len();
    while remaining > 0 {
        let num = remaining.min(LPDU_DATA_BLOCK_SIZE);
        dest[written..written + num].copy_from_slice(&src[read..read + num]);
        crc::add_crc(&mut dest[written..], num);
        read += num;
        written += num + LPDU_CRC_SIZE;
        remaining -= num;
    }
}

#[allow(clippy::too_many_arguments)]
fn format_header(
    cursor: &mut WriteCursor,
    data_length: u8,
    is_master: bool,
    fcb: bool,
    fcvdfc: bool,
    func: LinkFunction,
    dest: u16,
    src: u16,
) {
    let header = LinkHeader::new(
        data_length + LPDU_MIN_LENGTH,
        src,
        dest,
        is_master,
        fcvdfc,
        fcb,
        func,
    );
    header.write(cursor.unwritten());
    // safety: caller guarantees >= 10 bytes remain
    cursor.advance(LPDU_HEADER_SIZE).expect("header space");
    tracing::trace!(
        func = %func,
        dest,
        src,
        data_length,
        "link TX"
    );
}

// ---- Secondary -> Primary formats ----

pub fn format_ack(
    cursor: &mut WriteCursor,
    is_master: bool,
    rcv_buff_full: bool,
    dest: u16,
    src: u16,
) {
    format_header(
        cursor,
        0,
        is_master,
        false,
        rcv_buff_full,
        LinkFunction::SecAck,
        dest,
        src,
    );
}

pub fn format_nack(
    cursor: &mut WriteCursor,
    is_master: bool,
    rcv_buff_full: bool,
    dest: u16,
    src: u16,
) {
    format_header(
        cursor,
        0,
        is_master,
        false,
        rcv_buff_full,
        LinkFunction::SecNack,
        dest,
        src,
    );
}

pub fn format_link_status(
    cursor: &mut WriteCursor,
    is_master: bool,
    rcv_buff_full: bool,
    dest: u16,
    src: u16,
) {
    format_header(
        cursor,
        0,
        is_master,
        false,
        rcv_buff_full,
        LinkFunction::SecLinkStatus,
        dest,
        src,
    );
}

pub fn format_not_supported(
    cursor: &mut WriteCursor,
    is_master: bool,
    rcv_buff_full: bool,
    dest: u16,
    src: u16,
) {
    format_header(
        cursor,
        0,
        is_master,
        false,
        rcv_buff_full,
        LinkFunction::SecNotSupported,
        dest,
        src,
    );
}

// ---- Primary -> Secondary formats ----

pub fn format_reset_link_states(cursor: &mut WriteCursor, is_master: bool, dest: u16, src: u16) {
    format_header(
        cursor,
        0,
        is_master,
        false,
        false,
        LinkFunction::PriResetLinkStates,
        dest,
        src,
    );
}

pub fn format_request_link_status(cursor: &mut WriteCursor, is_master: bool, dest: u16, src: u16) {
    format_header(
        cursor,
        0,
        is_master,
        false,
        false,
        LinkFunction::PriRequestLinkStatus,
        dest,
        src,
    );
}

pub fn format_test_link_status(
    cursor: &mut WriteCursor,
    is_master: bool,
    fcb: bool,
    dest: u16,
    src: u16,
) {
    format_header(
        cursor,
        0,
        is_master,
        fcb,
        true,
        LinkFunction::PriTestLinkStates,
        dest,
        src,
    );
}

/// Format a confirmed-user-data frame. Returns `None` if `user_data` exceeds
/// [`LPDU_MAX_USER_DATA_SIZE`].
pub fn format_confirmed_user_data(
    cursor: &mut WriteCursor,
    is_master: bool,
    fcb: bool,
    dest: u16,
    src: u16,
    user_data: &[u8],
) -> Option<()> {
    if user_data.len() > LPDU_MAX_USER_DATA_SIZE {
        return None;
    }
    format_header(
        cursor,
        user_data.len() as u8,
        is_master,
        fcb,
        true,
        LinkFunction::PriConfirmedUserData,
        dest,
        src,
    );
    let size = calc_user_data_size(user_data.len());
    write_user_data(user_data, cursor.unwritten());
    cursor.advance(size).expect("user data space");
    Some(())
}

/// Format an unconfirmed-user-data frame. Returns `None` if `user_data`
/// exceeds [`LPDU_MAX_USER_DATA_SIZE`].
pub fn format_unconfirmed_user_data(
    cursor: &mut WriteCursor,
    is_master: bool,
    dest: u16,
    src: u16,
    user_data: &[u8],
) -> Option<()> {
    if user_data.len() > LPDU_MAX_USER_DATA_SIZE {
        return None;
    }
    format_header(
        cursor,
        user_data.len() as u8,
        is_master,
        false,
        false,
        LinkFunction::PriUnconfirmedUserData,
        dest,
        src,
    );
    let size = calc_user_data_size(user_data.len());
    write_user_data(user_data, cursor.unwritten());
    cursor.advance(size).expect("user data space");
    Some(())
}
