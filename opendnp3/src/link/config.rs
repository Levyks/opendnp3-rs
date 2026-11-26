//! Link-layer configuration.

use std::time::Duration;

/// Configuration for the DNP3 link layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkConfig {
    /// The master/outstation bit set on all transmitted frames.
    pub is_master: bool,
    /// DNP3 address of the local device.
    pub local_addr: u16,
    /// DNP3 address of the remote device.
    pub remote_addr: u16,
    /// Response timeout for confirmed requests.
    pub timeout: Duration,
    /// Interval for keep-alive messages (link status requests). Set to
    /// [`Duration::MAX`] to disable keep-alives.
    pub keep_alive_timeout: Duration,
    /// If true, the link layer responds to frames from any source address
    /// (used for multidrop outstation discovery).
    pub respond_to_any_source: bool,
}

impl LinkConfig {
    pub fn new(is_master: bool) -> Self {
        Self {
            is_master,
            local_addr: if is_master { 1 } else { 1024 },
            remote_addr: if is_master { 1024 } else { 1 },
            timeout: Duration::from_secs(1),
            keep_alive_timeout: Duration::from_secs(60),
            respond_to_any_source: false,
        }
    }
}
