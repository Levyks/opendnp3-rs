//! Application-layer control field (AC) and 4-bit sequence number.

const FIR_MASK: u8 = 0x80;
const FIN_MASK: u8 = 0x40;
const CON_MASK: u8 = 0x20;
const UNS_MASK: u8 = 0x10;
const SEQ_MASK: u8 = 0x0F;

/// Default maximum APDU fragment size (2048 bytes per IEEE 1815).
pub const DEFAULT_MAX_APDU_SIZE: usize = 2048;

/// Application-layer 4-bit sequence number (0..=15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppSeqNum(u8);

impl AppSeqNum {
    pub const fn new(value: u8) -> Self {
        Self(value & SEQ_MASK)
    }

    pub const fn value(self) -> u8 {
        self.0
    }

    pub const fn next(self) -> Self {
        Self((self.0 + 1) & SEQ_MASK)
    }

    pub fn increment(&mut self) {
        self.0 = (self.0 + 1) & SEQ_MASK;
    }
}

/// Application Control octet (`FIR | FIN | CON | UNS | SEQ[3:0]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppControlField {
    pub fir: bool,
    pub fin: bool,
    pub con: bool,
    pub uns: bool,
    pub seq: u8,
}

impl AppControlField {
    pub const fn new(fir: bool, fin: bool, con: bool, uns: bool, seq: u8) -> Self {
        Self {
            fir,
            fin,
            con,
            uns,
            seq: seq & SEQ_MASK,
        }
    }

    pub const fn request(seq: u8) -> Self {
        Self::new(true, true, false, false, seq)
    }

    pub const fn from_byte(byte: u8) -> Self {
        Self {
            fir: (byte & FIR_MASK) != 0,
            fin: (byte & FIN_MASK) != 0,
            con: (byte & CON_MASK) != 0,
            uns: (byte & UNS_MASK) != 0,
            seq: byte & SEQ_MASK,
        }
    }

    pub const fn to_byte(self) -> u8 {
        let mut b = self.seq & SEQ_MASK;
        if self.fir {
            b |= FIR_MASK;
        }
        if self.fin {
            b |= FIN_MASK;
        }
        if self.con {
            b |= CON_MASK;
        }
        if self.uns {
            b |= UNS_MASK;
        }
        b
    }

    pub const fn is_fir_and_fin(self) -> bool {
        self.fir && self.fin
    }
}

impl Default for AppControlField {
    fn default() -> Self {
        Self::new(true, true, false, false, 0)
    }
}
