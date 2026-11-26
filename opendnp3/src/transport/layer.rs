//! Transport layer glue between the link layer and the application layer.

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::rx::TransportRx;
use super::tx::TransportTx;
use super::{MAX_TPDU_PAYLOAD, TransportSeqNum};
use crate::clock::MonotonicTime;
use crate::link::{LinkEvent, LinkLayer};
use crate::message::Message;

/// Events produced by the transport layer for the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportEvent {
    /// A complete, reassembled APDU arrived.
    Message(Message),
    /// A transmit operation completed; more may be sent.
    TxReady,
    /// The link layer came up / went down.
    LowerLayerUp,
    LowerLayerDown,
}

/// Sits between the link layer and the application layer: segments outbound
/// APDUs into TPDUs, reassembles inbound TPDUs into APDUs.
pub struct TransportLayer {
    is_online: bool,
    is_sending: bool,
    tx_seq: TransportSeqNum,
    pub rx: TransportRx,
    events: Vec<TransportEvent>,
}

impl TransportLayer {
    pub fn new(max_rx_frag_size: usize) -> Self {
        Self {
            is_online: false,
            is_sending: false,
            tx_seq: TransportSeqNum::default(),
            rx: TransportRx::new(max_rx_frag_size),
            events: Vec::new(),
        }
    }

    pub fn is_online(&self) -> bool {
        self.is_online
    }

    pub fn is_sending(&self) -> bool {
        self.is_sending
    }

    /// Drain events produced since the last call.
    pub fn drain_events(&mut self) -> Vec<TransportEvent> {
        core::mem::take(&mut self.events)
    }

    /// Begin transmitting an APDU through the link layer.
    pub fn begin_transmit(
        &mut self,
        message: Message,
        link: &mut LinkLayer,
        now: MonotonicTime,
    ) -> bool {
        if !self.is_online {
            tracing::error!("transport layer offline");
            return false;
        }
        if message.payload.is_empty() {
            tracing::error!("APDU cannot be empty");
            return false;
        }
        if self.is_sending {
            tracing::error!("invalid begin_transmit call, already transmitting");
            return false;
        }
        self.is_sending = true;
        let num_segments = message.payload.len().div_ceil(MAX_TPDU_PAYLOAD);
        let start_seq = self.tx_seq;
        for _ in 0..num_segments {
            self.tx_seq.increment();
        }
        link.send(
            Box::new(TransportTx::with_sequence(message, start_seq)),
            now,
        );
        true
    }

    /// Feed a link-layer event through the transport layer, generating
    /// transport events for the application layer.
    pub fn handle_link_event(&mut self, event: &LinkEvent) {
        match event {
            LinkEvent::Message(message) => {
                self.on_receive(message);
            }
            LinkEvent::TxReady => {
                self.on_tx_ready();
            }
            LinkEvent::LowerLayerUp => {
                self.on_lower_layer_up();
            }
            LinkEvent::LowerLayerDown => {
                self.on_lower_layer_down();
            }
            _ => {}
        }
    }

    fn on_receive(&mut self, message: &Message) {
        if !self.is_online {
            tracing::error!("transport layer offline");
            return;
        }
        if let Some(asdu) = self.rx.process_receive(message) {
            self.events.push(TransportEvent::Message(asdu));
        }
    }

    fn on_tx_ready(&mut self) {
        if !self.is_online {
            tracing::error!("transport layer offline");
            return;
        }
        if !self.is_sending {
            tracing::error!("invalid send callback");
            return;
        }
        self.is_sending = false;
        self.events.push(TransportEvent::TxReady);
    }

    fn on_lower_layer_up(&mut self) {
        if self.is_online {
            tracing::error!("transport layer already online");
            return;
        }
        self.is_online = true;
        self.events.push(TransportEvent::LowerLayerUp);
    }

    fn on_lower_layer_down(&mut self) {
        if !self.is_online {
            tracing::error!("transport layer already offline");
            return;
        }
        self.is_online = false;
        self.is_sending = false;
        self.rx.reset();
        self.events.push(TransportEvent::LowerLayerDown);
    }
}
