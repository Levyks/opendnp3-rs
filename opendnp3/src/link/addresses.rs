//! Link-layer addressing.

use super::constants::LinkBroadcastAddress;

/// Source/destination pair carried in the frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Addresses {
    pub source: u16,
    pub destination: u16,
}

impl Addresses {
    pub fn new(source: u16, destination: u16) -> Self {
        Self {
            source,
            destination,
        }
    }

    /// Swap source and destination.
    pub fn reverse(&self) -> Self {
        Self {
            source: self.destination,
            destination: self.source,
        }
    }

    /// Is the destination a broadcast address?
    pub fn is_broadcast(&self) -> bool {
        LinkBroadcastAddress::is_broadcast(self.destination)
    }
}
