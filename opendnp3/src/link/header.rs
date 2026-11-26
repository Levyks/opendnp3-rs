//! FT3 frame header (10 bytes: `05 64 LEN CTRL DEST[2] SRC[2] CRC[2]`).

use super::constants::*;
use super::crc;
use crate::enums::LinkFunction;

/// Parsed/constructed link-layer frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkHeader {
    /// Length field: `[5, 255]` — 5 fixed octets + user data length.
    pub length: u8,
    /// Source address.
    pub src: u16,
    /// Destination address.
    pub dest: u16,
    /// Control octet.
    pub ctrl: u8,
}

impl LinkHeader {
    pub fn new(
        length: u8,
        src: u16,
        dest: u16,
        from_master: bool,
        fcvdfc: bool,
        fcb: bool,
        func: LinkFunction,
    ) -> Self {
        Self {
            length,
            src,
            dest,
            ctrl: Self::control_byte(from_master, fcb, fcvdfc, func),
        }
    }

    pub fn control(&self) -> u8 {
        self.ctrl
    }
    pub fn is_from_master(&self) -> bool {
        self.ctrl & MASK_DIR != 0
    }
    pub fn is_pri_to_sec(&self) -> bool {
        self.ctrl & MASK_PRM != 0
    }
    pub fn is_fcb_set(&self) -> bool {
        self.ctrl & MASK_FCB != 0
    }
    pub fn is_fcv_dfc_set(&self) -> bool {
        self.ctrl & MASK_FCV != 0
    }
    pub fn func(&self) -> LinkFunction {
        LinkFunction::from_u8(self.ctrl & MASK_FUNC_OR_PRM)
    }
    pub fn valid_length(&self) -> bool {
        self.length > 4
    }

    pub fn change_fcb(&mut self, fcb: bool) {
        if fcb {
            self.ctrl |= MASK_FCB;
        } else {
            self.ctrl &= !MASK_FCB;
        }
    }

    /// Build the control octet from fields.
    pub fn control_byte(is_master: bool, fcb: bool, fcvdfc: bool, func: LinkFunction) -> u8 {
        let mut ret = func.to_u8();
        if is_master {
            ret |= MASK_DIR;
        }
        if fcb {
            ret |= MASK_FCB;
        }
        if fcvdfc {
            ret |= MASK_FCV;
        }
        ret
    }

    /// Read a header from a buffer of at least [`LPDU_HEADER_SIZE`] bytes.
    /// Does NOT validate `0x0564` or the CRC.
    pub fn read(buf: &[u8]) -> Self {
        Self {
            length: buf[LI_LENGTH],
            dest: u16::from_le_bytes([buf[LI_DESTINATION], buf[LI_DESTINATION + 1]]),
            src: u16::from_le_bytes([buf[LI_SOURCE], buf[LI_SOURCE + 1]]),
            ctrl: buf[LI_CONTROL],
        }
    }

    /// Write the header (start bytes, fields, CRC) into a buffer of at least
    /// [`LPDU_HEADER_SIZE`] bytes.
    pub fn write(&self, buf: &mut [u8]) {
        buf[0] = 0x05;
        buf[1] = 0x64;
        buf[LI_LENGTH] = self.length;
        buf[LI_DESTINATION..LI_DESTINATION + 2].copy_from_slice(&self.dest.to_le_bytes());
        buf[LI_SOURCE..LI_SOURCE + 2].copy_from_slice(&self.src.to_le_bytes());
        buf[LI_CONTROL] = self.ctrl;
        crc::add_crc(buf, LI_CRC);
    }
}
