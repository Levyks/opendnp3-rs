//! Streaming link-frame parser: resync, header validation, body CRC checks.

use super::addresses::Addresses;
use super::constants::*;
use super::crc;
use super::frame;
use super::header::LinkHeader;
use crate::enums::LinkFunction;

/// Fields of a received frame's header, pre-digested for the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkHeaderFields {
    pub func: LinkFunction,
    pub is_from_master: bool,
    pub fcb: bool,
    pub fcvdfc: bool,
    pub addresses: Addresses,
}

/// A complete, CRC-validated received frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedFrame {
    pub fields: LinkHeaderFields,
    /// User-data payload (empty for header-only frames).
    pub payload: Vec<u8>,
}

/// Parser error counters (mirrors the C++ `LinkLayerStatistics` subset).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParserStatistics {
    pub num_header_crc_error: u64,
    pub num_body_crc_error: u64,
    pub num_bad_length: u64,
    pub num_bad_function_code: u64,
    pub num_bad_fcv: u64,
    pub num_bad_fcb: u64,
    pub num_frame_rx: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    FindSync,
    ReadHeader,
    ReadBody,
}

/// Incremental parser. Feed raw bytes with [`push_bytes`](Self::push_bytes);
/// complete validated frames come back out.
pub struct FrameParser {
    buffer: [u8; LPDU_MAX_FRAME_SIZE],
    read_pos: usize,
    write_pos: usize,
    state: State,
    header: LinkHeader,
    frame_size: usize,
    pub statistics: ParserStatistics,
}

impl Default for FrameParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameParser {
    pub fn new() -> Self {
        Self {
            buffer: [0; LPDU_MAX_FRAME_SIZE],
            read_pos: 0,
            write_pos: 0,
            state: State::FindSync,
            header: LinkHeader {
                length: 0,
                src: 0,
                dest: 0,
                ctrl: 0,
            },
            frame_size: 0,
            statistics: ParserStatistics::default(),
        }
    }

    pub fn reset(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.state = State::FindSync;
        self.frame_size = 0;
    }

    /// Number of buffered-but-unconsumed bytes.
    fn num_read(&self) -> usize {
        self.write_pos - self.read_pos
    }

    /// Feed raw bytes; returns all complete frames parsed.
    pub fn push_bytes(&mut self, mut bytes: &[u8]) -> Vec<ReceivedFrame> {
        let mut frames = Vec::new();
        while !bytes.is_empty() {
            // shift buffered data to the front if we're low on write space
            if self.write_pos + bytes.len() > self.buffer.len() {
                self.buffer.copy_within(self.read_pos..self.write_pos, 0);
                self.write_pos -= self.read_pos;
                self.read_pos = 0;
            }
            let n = bytes.len().min(self.buffer.len() - self.write_pos);
            self.buffer[self.write_pos..self.write_pos + n].copy_from_slice(&bytes[..n]);
            self.write_pos += n;
            bytes = &bytes[n..];
            self.parse(&mut frames);
        }
        frames
    }

    fn parse(&mut self, frames: &mut Vec<ReceivedFrame>) {
        loop {
            match self.state {
                State::FindSync => {
                    if self.num_read() < 2 {
                        return;
                    }
                    // scan for 0x05 0x64
                    let mut synced = false;
                    while self.num_read() > 1 {
                        if self.buffer[self.read_pos] == 0x05
                            && self.buffer[self.read_pos + 1] == 0x64
                        {
                            synced = true;
                            break;
                        }
                        self.read_pos += 1;
                    }
                    if !synced {
                        return;
                    }
                    self.state = State::ReadHeader;
                }
                State::ReadHeader => {
                    if self.num_read() < LPDU_HEADER_SIZE {
                        return;
                    }
                    match self.read_header() {
                        Ok(()) => self.state = State::ReadBody,
                        Err(()) => {
                            self.fail_frame();
                            self.state = State::FindSync;
                        }
                    }
                }
                State::ReadBody => {
                    if self.num_read() < self.frame_size {
                        return;
                    }
                    if self.validate_body() {
                        let frame = self.take_frame();
                        self.statistics.num_frame_rx += 1;
                        frames.push(frame);
                        self.state = State::FindSync;
                    } else {
                        self.fail_frame();
                        self.state = State::FindSync;
                    }
                }
            }
        }
    }

    fn read_header(&mut self) -> Result<(), ()> {
        let buf = &self.buffer[self.read_pos..self.read_pos + LPDU_HEADER_SIZE];
        if !crc::is_correct_crc(buf, LI_CRC) {
            self.statistics.num_header_crc_error += 1;
            tracing::warn!("CRC failure in link header");
            return Err(());
        }
        self.header = LinkHeader::read(buf);
        self.validate_header_parameters()
    }

    fn validate_header_parameters(&mut self) -> Result<(), ()> {
        if !self.header.valid_length() {
            self.statistics.num_bad_length += 1;
            tracing::warn!(length = self.header.length, "LENGTH out of range [5,255]");
            return Err(());
        }
        self.validate_function_code()?;

        let user_data_length = self.header.length - LPDU_MIN_LENGTH;
        self.frame_size = frame::calc_frame_size(user_data_length as usize);
        let func = self.header.func();
        let has_payload = user_data_length > 0;
        let should_have_payload = matches!(
            func,
            LinkFunction::PriConfirmedUserData | LinkFunction::PriUnconfirmedUserData
        );

        if should_have_payload != has_payload {
            self.statistics.num_bad_length += 1;
            tracing::warn!(%func, "payload presence does not match function code");
            return Err(());
        }
        Ok(())
    }

    fn validate_function_code(&mut self) -> Result<(), ()> {
        if self.header.is_pri_to_sec() {
            let fcv_set = match self.header.func() {
                LinkFunction::PriConfirmedUserData | LinkFunction::PriTestLinkStates => true,
                LinkFunction::PriRequestLinkStatus
                | LinkFunction::PriResetLinkStates
                | LinkFunction::PriUnconfirmedUserData => false,
                other => {
                    self.statistics.num_bad_function_code += 1;
                    tracing::warn!(func = %other, "unknown pri-to-sec function");
                    return Err(());
                }
            };
            if fcv_set != self.header.is_fcv_dfc_set() {
                self.statistics.num_bad_fcv += 1;
                tracing::warn!(func = %self.header.func(), "bad FCV for function");
                return Err(());
            }
        } else {
            match self.header.func() {
                LinkFunction::SecAck
                | LinkFunction::SecNack
                | LinkFunction::SecLinkStatus
                | LinkFunction::SecNotSupported => {}
                other => {
                    self.statistics.num_bad_function_code += 1;
                    tracing::warn!(func = %other, "unknown sec-to-pri function");
                    return Err(());
                }
            }
            if self.header.is_fcb_set() {
                self.statistics.num_bad_fcb += 1;
                tracing::warn!(func = %self.header.func(), "FCB set for sec-to-pri function");
                return Err(());
            }
        }
        Ok(())
    }

    fn validate_body(&mut self) -> bool {
        let user_len = (self.header.length - LPDU_MIN_LENGTH) as usize;
        let body_start = self.read_pos + LPDU_HEADER_SIZE;
        let body = &self.buffer[body_start..body_start + self.frame_size - LPDU_HEADER_SIZE];
        if frame::validate_body_crc(body, user_len) {
            tracing::trace!(
                func = %self.header.func(),
                dest = self.header.dest,
                src = self.header.src,
                length = self.header.length,
                "link RX"
            );
            true
        } else {
            self.statistics.num_body_crc_error += 1;
            tracing::warn!("CRC failure in link body");
            false
        }
    }

    fn take_frame(&mut self) -> ReceivedFrame {
        let user_len = (self.header.length - LPDU_MIN_LENGTH) as usize;
        let mut payload = vec![0u8; user_len];
        if user_len > 0 {
            let body_start = self.read_pos + LPDU_HEADER_SIZE;
            let body = &self.buffer[body_start..body_start + self.frame_size - LPDU_HEADER_SIZE];
            frame::read_user_data(body, &mut payload);
        }
        let frame = ReceivedFrame {
            fields: LinkHeaderFields {
                func: self.header.func(),
                is_from_master: self.header.is_from_master(),
                fcb: self.header.is_fcb_set(),
                fcvdfc: self.header.is_fcv_dfc_set(),
                addresses: Addresses::new(self.header.src, self.header.dest),
            },
            payload,
        };
        self.read_pos += self.frame_size;
        frame
    }

    /// Discard one byte to allow resync (C++ `FailFrame`).
    fn fail_frame(&mut self) {
        self.read_pos += 1;
    }
}
