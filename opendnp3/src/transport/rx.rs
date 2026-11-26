//! Transport receive: TPDU -> APDU reassembly.

use alloc::vec;
use alloc::vec::Vec;

use super::{TransportHeader, TransportSeqNum};
use crate::link::Addresses;
use crate::message::Message;

/// RX statistics (C++ `StackStatistics::Transport::Rx`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RxStatistics {
    pub num_transport_rx: u64,
    pub num_transport_error_rx: u64,
    pub num_transport_discard: u64,
    pub num_transport_ignore: u64,
    pub num_transport_buffer_overflow: u64,
}

/// Reassembles TPDUs into complete APDUs.
pub struct TransportRx {
    rx_buffer: Vec<u8>,
    num_bytes_read: usize,
    expected_seq: TransportSeqNum,
    last_addresses: Addresses,
    pub statistics: RxStatistics,
}

impl TransportRx {
    pub fn new(max_rx_frag_size: usize) -> Self {
        Self {
            rx_buffer: vec![0u8; max_rx_frag_size],
            num_bytes_read: 0,
            expected_seq: TransportSeqNum::default(),
            last_addresses: Addresses::default(),
            statistics: RxStatistics::default(),
        }
    }

    pub fn reset(&mut self) {
        self.num_bytes_read = 0;
    }

    /// Process a received LSDU (TPDU). Returns the complete APDU when a FIN
    /// segment completes a fragment, otherwise `None`.
    pub fn process_receive(&mut self, segment: &Message) -> Option<Message> {
        self.statistics.num_transport_rx += 1;

        if segment.payload.is_empty() {
            tracing::warn!("received TPDU with no header");
            self.statistics.num_transport_error_rx += 1;
            return None;
        }

        let header = TransportHeader::from_byte(segment.payload[0]);
        let payload = &segment.payload[1..];

        tracing::trace!(
            fir = header.fir,
            fin = header.fin,
            seq = header.seq,
            len = payload.len(),
            "transport RX"
        );

        if header.fir && self.num_bytes_read > 0 {
            self.statistics.num_transport_discard += 1;
            tracing::warn!("FIR received mid-fragment, discarding previous bytes");
            self.num_bytes_read = 0;
        }

        if !header.fir {
            if self.num_bytes_read == 0 {
                self.statistics.num_transport_ignore += 1;
                tracing::warn!("non-FIR packet with 0 prior bytes");
                return None;
            }
            if header.seq != self.expected_seq.get() {
                self.statistics.num_transport_ignore += 1;
                tracing::warn!(
                    seq = header.seq,
                    expected = self.expected_seq.get(),
                    "received segment with unexpected seq"
                );
                return None;
            }
            if segment.addresses != self.last_addresses {
                self.statistics.num_transport_ignore += 1;
                tracing::warn!(
                    last_src = self.last_addresses.source,
                    last_dest = self.last_addresses.destination,
                    src = segment.addresses.source,
                    dest = segment.addresses.destination,
                    "bad addressing on continuation segment"
                );
                return None;
            }
        }

        if payload.len() > self.rx_buffer.len() - self.num_bytes_read {
            self.statistics.num_transport_buffer_overflow += 1;
            tracing::warn!("exceeded buffer size before complete fragment read");
            self.num_bytes_read = 0;
            return None;
        }

        self.rx_buffer[self.num_bytes_read..self.num_bytes_read + payload.len()]
            .copy_from_slice(payload);
        self.num_bytes_read += payload.len();
        self.last_addresses = segment.addresses;
        self.expected_seq = TransportSeqNum::new(header.seq);
        self.expected_seq.increment();

        if header.fin {
            let data = self.rx_buffer[..self.num_bytes_read].to_vec();
            self.num_bytes_read = 0;
            return Some(Message::new(segment.addresses, data));
        }

        None
    }
}
