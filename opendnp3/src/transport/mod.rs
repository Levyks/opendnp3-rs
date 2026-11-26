//! DNP3 transport layer: TPDU segmentation and reassembly.

pub mod layer;
pub mod rx;
pub mod tx;

pub use layer::{TransportEvent, TransportLayer};
pub use rx::TransportRx;
pub use tx::TransportTx;

/// Maximum TPDU length including the 1-byte header.
pub const MAX_TPDU_LENGTH: usize = 250;
/// Maximum TPDU payload size.
pub const MAX_TPDU_PAYLOAD: usize = 249;

const FIN_MASK: u8 = 0x80;
const FIR_MASK: u8 = 0x40;
const SEQ_MASK: u8 = 0x3F;

/// The 1-byte transport header (FIR/FIN + 6-bit sequence).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportHeader {
    pub fir: bool,
    pub fin: bool,
    pub seq: u8,
}

impl TransportHeader {
    pub fn from_byte(byte: u8) -> Self {
        Self {
            fir: byte & FIR_MASK != 0,
            fin: byte & FIN_MASK != 0,
            seq: byte & SEQ_MASK,
        }
    }

    pub fn to_byte(fir: bool, fin: bool, seq: u8) -> u8 {
        let mut hdr = 0u8;
        if fir {
            hdr |= FIR_MASK;
        }
        if fin {
            hdr |= FIN_MASK;
        }
        hdr | (seq & SEQ_MASK)
    }
}

/// A 6-bit transport sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransportSeqNum(u8);

impl TransportSeqNum {
    pub fn new(value: u8) -> Self {
        Self(value & SEQ_MASK)
    }

    pub fn get(self) -> u8 {
        self.0
    }

    pub fn increment(&mut self) {
        self.0 = (self.0 + 1) & SEQ_MASK;
    }
}
