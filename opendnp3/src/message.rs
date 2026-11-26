//! An addressed payload passed between protocol layers.

use alloc::vec::Vec;

use crate::link::Addresses;

/// An addressed unit of user data (LSDU at the link layer, APDU at the
/// application layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub addresses: Addresses,
    pub payload: Vec<u8>,
}

impl Message {
    pub fn new(addresses: Addresses, payload: Vec<u8>) -> Self {
        Self { addresses, payload }
    }
}
