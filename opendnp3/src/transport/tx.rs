//! Transport transmit: APDU -> TPDU segmentation.

use super::{MAX_TPDU_LENGTH, MAX_TPDU_PAYLOAD, TransportHeader, TransportSeqNum};
use crate::link::{Addresses, TransportSegment};
use crate::message::Message;

/// TX statistics (C++ `StackStatistics::Transport::Tx`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TxStatistics {
    pub num_transport_tx: u64,
}

/// Segments an APDU into TPDUs for link-layer transmission.
/// Implements [`TransportSegment`] and is moved into the link layer for the
/// duration of a send.
pub struct TransportTx {
    message: Message,
    offset: usize,
    segment_buf: [u8; MAX_TPDU_LENGTH],
    segment_len: usize,
    has_segment: bool,
    sequence: TransportSeqNum,
    tpdu_count: u32,
    pub statistics: TxStatistics,
}

impl TransportTx {
    pub fn new(message: Message) -> Self {
        Self::with_sequence(message, TransportSeqNum::default())
    }

    pub fn with_sequence(message: Message, sequence: TransportSeqNum) -> Self {
        debug_assert!(!message.payload.is_empty());
        Self {
            message,
            offset: 0,
            segment_buf: [0; MAX_TPDU_LENGTH],
            segment_len: 0,
            has_segment: false,
            sequence,
            tpdu_count: 0,
            statistics: TxStatistics::default(),
        }
    }
}

impl TransportSegment for TransportTx {
    fn addresses(&self) -> Addresses {
        self.message.addresses
    }

    fn has_value(&self) -> bool {
        self.offset < self.message.payload.len()
    }

    fn segment(&mut self) -> &[u8] {
        if self.has_segment {
            return &self.segment_buf[..self.segment_len];
        }

        let remaining = &self.message.payload[self.offset..];
        let num_to_send = remaining.len().min(MAX_TPDU_PAYLOAD);

        self.segment_buf[1..1 + num_to_send].copy_from_slice(&remaining[..num_to_send]);

        let fir = self.tpdu_count == 0;
        let fin = num_to_send == remaining.len();
        self.segment_buf[0] = TransportHeader::to_byte(fir, fin, self.sequence.get());

        tracing::trace!(
            fir,
            fin,
            seq = self.sequence.get(),
            len = num_to_send,
            "transport TX"
        );

        self.statistics.num_transport_tx += 1;
        self.segment_len = num_to_send + 1;
        self.has_segment = true;
        &self.segment_buf[..self.segment_len]
    }

    fn advance(&mut self) -> bool {
        self.has_segment = false;
        let remaining = self.message.payload.len() - self.offset;
        let num_to_send = remaining.min(MAX_TPDU_PAYLOAD);
        self.offset += num_to_send;
        self.tpdu_count += 1;
        self.sequence.increment();
        self.offset < self.message.payload.len()
    }
}
