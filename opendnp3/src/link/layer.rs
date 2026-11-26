//! The link-layer protocol state machine (port of C++ `LinkContext` +
//! `PriLinkLayerStates`/`SecLinkLayerStates`).
//!
//! The layer is event-driven: callers push frames/timeouts in, drain
//! [`LinkEvent`]s for the layer above, and pop serialized frames for the
//! channel below. No I/O or clock access happens inside.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use super::addresses::Addresses;
use super::config::LinkConfig;
use super::constants::*;
use super::frame;
use super::parser::LinkHeaderFields;
use crate::clock::MonotonicTime;
use crate::enums::LinkStatus;
use crate::message::Message;
use crate::util::WriteCursor;

/// Produces transport-layer segments (TPDUs) for transmission, one link
/// frame at a time. Port of C++ `ITransportSegment`.
pub trait TransportSegment: Send {
    fn addresses(&self) -> Addresses;
    fn has_value(&self) -> bool;
    /// The current segment (max 250 bytes).
    fn segment(&mut self) -> &[u8];
    /// Move to the next segment; returns true if more remain.
    fn advance(&mut self) -> bool;
}

/// Events produced by the link layer for the layer above (transport) and
/// the application (listener notifications).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    /// A validated data message arrived (user data pushed up).
    Message(Message),
    /// The channel came up / went down.
    LowerLayerUp,
    LowerLayerDown,
    /// Link state changed (reset/unreset).
    StateChange(LinkStatus),
    /// A queued transmission finished; upper layer may send more.
    TxReady,
    /// Keep-alive timer elapsed; a link status request was sent.
    KeepAliveInitiated,
    /// A keep-alive request received a valid response.
    KeepAliveSuccess,
    /// A keep-alive request timed out (or failed) — only `timeout=true`
    /// produces this in the C++ code.
    KeepAliveFailure,
    /// Frame received for an unexpected destination address.
    UnknownDestination(u16),
    /// Frame received from an unexpected source address.
    UnknownSource(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriState {
    Idle,
    SendUnconfirmedTransmitWait,
    RequestLinkStatusWait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecState {
    NotReset,
    Reset,
    TransmitWaitReset,
    TransmitWaitNotReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxMode {
    Idle,
    Primary,
    Secondary,
}

/// Link-layer statistics (subset of C++ `StackStatistics::Link`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStatistics {
    pub num_bad_master_bit: u64,
    pub num_unknown_destination: u64,
    pub num_unknown_source: u64,
    pub num_unexpected_frame: u64,
}

/// The link layer: framing state machine plus primary/secondary services.
pub struct LinkLayer {
    config: LinkConfig,

    // state
    pri_state: PriState,
    sec_state: SecState,
    tx_mode: TxMode,
    is_online: bool,
    next_read_fcb: bool,
    keep_alive_timeout: bool,

    // timers
    rsp_deadline: Option<MonotonicTime>,
    keep_alive_deadline: Option<MonotonicTime>,
    last_message_timestamp: MonotonicTime,

    // transmit path
    segments: Option<Box<dyn TransportSegment>>,
    pending_pri_tx: Option<Vec<u8>>,
    pending_sec_tx: Option<Vec<u8>>,
    ready_tx: Vec<Vec<u8>>,

    events: Vec<LinkEvent>,
    pub statistics: LinkStatistics,
}

impl LinkLayer {
    pub fn new(config: LinkConfig) -> Self {
        Self {
            config,
            pri_state: PriState::Idle,
            sec_state: SecState::NotReset,
            tx_mode: TxMode::Idle,
            is_online: false,
            next_read_fcb: false,
            keep_alive_timeout: false,
            rsp_deadline: None,
            keep_alive_deadline: None,
            last_message_timestamp: MonotonicTime::ZERO,
            segments: None,
            pending_pri_tx: None,
            pending_sec_tx: None,
            ready_tx: Vec::new(),
            events: Vec::new(),
            statistics: LinkStatistics::default(),
        }
    }

    pub fn is_online(&self) -> bool {
        self.is_online
    }

    /// Drain events generated since the last call.
    pub fn drain_events(&mut self) -> Vec<LinkEvent> {
        core::mem::take(&mut self.events)
    }

    /// Next timer deadline, if any. The driver should call
    /// [`on_timeout`](Self::on_timeout) at this time.
    pub fn next_wakeup(&self) -> Option<MonotonicTime> {
        match (self.rsp_deadline, self.keep_alive_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, b) => b,
        }
    }

    /// Pop the next frame to write to the channel, if any.
    pub fn pop_transmit(&mut self) -> Option<Vec<u8>> {
        if self.ready_tx.is_empty() {
            None
        } else {
            Some(self.ready_tx.remove(0))
        }
    }

    pub fn has_pending_transmit(&self) -> bool {
        !self.ready_tx.is_empty()
    }

    // ---- events from below ----

    pub fn on_lower_layer_up(&mut self, now: MonotonicTime) -> bool {
        if self.is_online {
            tracing::error!("link layer already online");
            return false;
        }
        self.is_online = true;
        self.restart_keep_alive_timer(now);
        self.events
            .push(LinkEvent::StateChange(LinkStatus::Unreset));
        self.events.push(LinkEvent::LowerLayerUp);
        true
    }

    pub fn on_lower_layer_down(&mut self) -> bool {
        if !self.is_online {
            tracing::error!("link layer is not online");
            return false;
        }
        self.is_online = false;
        self.keep_alive_timeout = false;
        self.segments = None;
        self.tx_mode = TxMode::Idle;
        self.pending_pri_tx = None;
        self.pending_sec_tx = None;
        self.ready_tx.clear();
        self.rsp_deadline = None;
        self.keep_alive_deadline = None;
        self.pri_state = PriState::Idle;
        self.sec_state = SecState::NotReset;
        self.events
            .push(LinkEvent::StateChange(LinkStatus::Unreset));
        self.events.push(LinkEvent::LowerLayerDown);
        true
    }

    /// The channel finished writing the current frame.
    pub fn on_tx_ready(&mut self) -> bool {
        if self.tx_mode == TxMode::Idle {
            tracing::error!("unknown transmission callback");
            return false;
        }
        let was_primary = self.tx_mode == TxMode::Primary;
        self.tx_mode = TxMode::Idle;

        // give pending transmissions first access to the transmitter
        self.try_pending_tx(false);
        self.try_pending_tx(true);

        if was_primary {
            self.pri_on_tx_ready();
        } else {
            self.sec_on_tx_ready();
        }
        true
    }

    /// A validated frame arrived from the parser.
    pub fn on_frame(
        &mut self,
        header: &LinkHeaderFields,
        userdata: &[u8],
        now: MonotonicTime,
    ) -> bool {
        if !self.is_online {
            tracing::error!("link layer is not online");
            return false;
        }
        if header.is_from_master == self.config.is_master {
            self.statistics.num_bad_master_bit += 1;
            tracing::warn!("frame received with matching master bit");
            return false;
        }
        if header.addresses.destination != self.config.local_addr
            && !header.addresses.is_broadcast()
        {
            self.statistics.num_unknown_destination += 1;
            self.events
                .push(LinkEvent::UnknownDestination(header.addresses.destination));
            return false;
        }
        if header.addresses.source != self.config.remote_addr && !self.config.respond_to_any_source
        {
            self.statistics.num_unknown_source += 1;
            self.events
                .push(LinkEvent::UnknownSource(header.addresses.source));
            return false;
        }

        // broadcast may only be used with data-carrying functions
        if header.addresses.is_broadcast()
            && !matches!(
                header.func,
                crate::enums::LinkFunction::PriUnconfirmedUserData
                    | crate::enums::LinkFunction::PriConfirmedUserData
            )
        {
            self.statistics.num_unexpected_frame += 1;
            tracing::warn!(func = %header.func, "invalid function for broadcast destination");
            return false;
        }

        self.restart_keep_alive_timer(now);

        use crate::enums::LinkFunction as LF;
        match header.func {
            LF::SecAck => self.pri_on_ack(header.fcvdfc),
            LF::SecNack => self.pri_on_nack(header.fcvdfc),
            LF::SecLinkStatus => self.pri_on_link_status(header.fcvdfc),
            LF::SecNotSupported => self.pri_on_not_supported(header.fcvdfc),
            LF::PriTestLinkStates => {
                self.sec_on_test_link_status(header.addresses.source, header.fcb)
            }
            LF::PriResetLinkStates => self.sec_on_reset_link_states(header.addresses.source),
            LF::PriRequestLinkStatus => self.sec_on_request_link_status(header.addresses.source),
            LF::PriConfirmedUserData => self.sec_on_confirmed_user_data(
                header.addresses.source,
                header.fcb,
                header.addresses.is_broadcast(),
                Message::new(header.addresses, userdata.to_vec()),
            ),
            LF::PriUnconfirmedUserData => {
                self.push_data_up(Message::new(header.addresses, userdata.to_vec()))
            }
            _ => {}
        }
        true
    }

    // ---- events from above ----

    /// Queue a transport-segment producer for transmission.
    pub fn send(&mut self, segments: Box<dyn TransportSegment>, now: MonotonicTime) -> bool {
        if !self.is_online {
            tracing::error!("link layer is not online");
            return false;
        }
        if self.segments.is_some() {
            tracing::error!("already transmitting a segment");
            return false;
        }
        self.segments = Some(segments);
        self.try_start_transmission(now);
        true
    }

    /// Timer driver: call when [`next_wakeup`](Self::next_wakeup) is reached.
    pub fn on_timeout(&mut self, now: MonotonicTime) {
        if let Some(deadline) = self.rsp_deadline
            && now >= deadline
        {
            self.rsp_deadline = None;
            self.on_response_timeout(now);
        }
        if let Some(deadline) = self.keep_alive_deadline
            && now >= deadline
        {
            self.on_keep_alive_timeout(now);
        }
    }

    // ---- internal helpers (LinkContext) ----

    fn push_data_up(&mut self, message: Message) {
        self.events.push(LinkEvent::Message(message));
    }

    fn complete_send_operation(&mut self) {
        self.segments = None;
        self.events.push(LinkEvent::TxReady);
    }

    fn format_primary_unconfirmed(&mut self, addresses: Addresses, tpdu: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; LPDU_MAX_FRAME_SIZE];
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_unconfirmed_user_data(
            &mut cursor,
            self.config.is_master,
            addresses.destination,
            addresses.source,
            tpdu,
        );
        cursor.into_written().to_vec()
    }

    fn queue_transmit(&mut self, frame: Vec<u8>, primary: bool) {
        if self.tx_mode == TxMode::Idle {
            self.tx_mode = if primary {
                TxMode::Primary
            } else {
                TxMode::Secondary
            };
            self.ready_tx.push(frame);
        } else if primary {
            self.pending_pri_tx = Some(frame);
        } else {
            self.pending_sec_tx = Some(frame);
        }
    }

    fn queue_ack(&mut self, destination: u16) {
        let mut buf = [0u8; LPDU_HEADER_SIZE];
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_ack(
            &mut cursor,
            self.config.is_master,
            false,
            destination,
            self.config.local_addr,
        );
        self.queue_transmit(buf.to_vec(), false);
    }

    fn queue_link_status(&mut self, destination: u16) {
        let mut buf = [0u8; LPDU_HEADER_SIZE];
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_link_status(
            &mut cursor,
            self.config.is_master,
            false,
            destination,
            self.config.local_addr,
        );
        self.queue_transmit(buf.to_vec(), false);
    }

    fn queue_request_link_status(&mut self, destination: u16) {
        let mut buf = [0u8; LPDU_HEADER_SIZE];
        let mut cursor = WriteCursor::new(&mut buf);
        frame::format_request_link_status(
            &mut cursor,
            self.config.is_master,
            destination,
            self.config.local_addr,
        );
        self.queue_transmit(buf.to_vec(), true);
    }

    fn try_start_transmission(&mut self, now: MonotonicTime) {
        if self.keep_alive_timeout {
            self.pri_try_send_request_link_status(now);
        }
        if self.segments.is_some() {
            self.pri_try_send_unconfirmed();
        }
    }

    fn on_keep_alive_timeout(&mut self, now: MonotonicTime) {
        let elapsed = now.saturating_duration_since(self.last_message_timestamp);
        if elapsed >= self.config.keep_alive_timeout {
            self.keep_alive_timeout = true;
        }
        self.restart_keep_alive_timer(now);
        self.try_start_transmission(now);
    }

    fn on_response_timeout(&mut self, now: MonotonicTime) {
        self.pri_on_timeout();
        self.try_start_transmission(now);
    }

    fn start_response_timer(&mut self, now: MonotonicTime) {
        self.rsp_deadline = Some(now + self.config.timeout);
    }

    fn restart_keep_alive_timer(&mut self, now: MonotonicTime) {
        self.last_message_timestamp = now;
        if self.config.keep_alive_timeout != Duration::MAX {
            self.keep_alive_deadline = Some(now + self.config.keep_alive_timeout);
        }
    }

    fn cancel_response_timer(&mut self) {
        self.rsp_deadline = None;
    }

    fn fail_keep_alive(&mut self, timeout: bool) {
        if timeout {
            self.events.push(LinkEvent::KeepAliveFailure);
        }
    }

    fn complete_keep_alive(&mut self) {
        self.events.push(LinkEvent::KeepAliveSuccess);
    }

    fn try_pending_tx(&mut self, primary: bool) -> bool {
        let pending = if primary {
            self.pending_pri_tx.take()
        } else {
            self.pending_sec_tx.take()
        };
        if self.tx_mode == TxMode::Idle
            && let Some(frame) = pending
        {
            self.tx_mode = if primary {
                TxMode::Primary
            } else {
                TxMode::Secondary
            };
            self.ready_tx.push(frame);
            return true;
        }
        false
    }

    // ---- primary station state machine ----

    fn pri_on_ack(&mut self, _rx_buff_full: bool) {
        match self.pri_state {
            PriState::RequestLinkStatusWait => {
                self.cancel_response_timer();
                self.fail_keep_alive(false);
                self.pri_state = PriState::Idle;
            }
            _ => {
                self.statistics.num_unexpected_frame += 1;
                tracing::warn!("frame context not understood (ACK)");
            }
        }
    }

    fn pri_on_nack(&mut self, _rx_buff_full: bool) {
        match self.pri_state {
            PriState::RequestLinkStatusWait => {
                self.cancel_response_timer();
                self.fail_keep_alive(false);
                self.pri_state = PriState::Idle;
            }
            _ => {
                self.statistics.num_unexpected_frame += 1;
                tracing::warn!("frame context not understood (NACK)");
            }
        }
    }

    fn pri_on_link_status(&mut self, _rx_buff_full: bool) {
        match self.pri_state {
            PriState::RequestLinkStatusWait => {
                self.cancel_response_timer();
                self.complete_keep_alive();
                self.pri_state = PriState::Idle;
            }
            _ => {
                self.statistics.num_unexpected_frame += 1;
                tracing::warn!("frame context not understood (LINK_STATUS)");
            }
        }
    }

    fn pri_on_not_supported(&mut self, _rx_buff_full: bool) {
        match self.pri_state {
            PriState::RequestLinkStatusWait => {
                self.cancel_response_timer();
                self.fail_keep_alive(false);
                self.pri_state = PriState::Idle;
            }
            _ => {
                self.statistics.num_unexpected_frame += 1;
                tracing::warn!("frame context not understood (NOT_SUPPORTED)");
            }
        }
    }

    fn pri_on_tx_ready(&mut self) {
        match self.pri_state {
            PriState::SendUnconfirmedTransmitWait => {
                let mut finished = false;
                let mut next_frame = None;
                if let Some(segments) = self.segments.as_mut() {
                    if segments.advance() {
                        let addresses = segments.addresses();
                        let tpdu = segments.segment().to_vec();
                        next_frame = Some(self.format_primary_unconfirmed(addresses, &tpdu));
                    } else {
                        finished = true;
                    }
                }
                if let Some(frame) = next_frame {
                    self.queue_transmit(frame, true);
                }
                if finished {
                    self.complete_send_operation();
                    self.pri_state = PriState::Idle;
                }
            }
            PriState::RequestLinkStatusWait => {
                // request link status was successfully sent
            }
            PriState::Idle => {
                tracing::error!("invalid action for state: Idle");
            }
        }
    }

    fn pri_on_timeout(&mut self) {
        match self.pri_state {
            PriState::RequestLinkStatusWait => {
                tracing::warn!("link status request - response timeout");
                self.fail_keep_alive(true);
                self.pri_state = PriState::Idle;
            }
            _ => {
                tracing::error!("invalid action for state");
            }
        }
    }

    fn pri_try_send_unconfirmed(&mut self) {
        if self.pri_state != PriState::Idle {
            return;
        }
        let (addresses, tpdu) = match self.segments.as_mut() {
            Some(s) => (s.addresses(), s.segment().to_vec()),
            None => return,
        };
        let frame = self.format_primary_unconfirmed(addresses, &tpdu);
        self.queue_transmit(frame, true);
        self.pri_state = PriState::SendUnconfirmedTransmitWait;
    }

    fn pri_try_send_request_link_status(&mut self, now: MonotonicTime) {
        if self.pri_state != PriState::Idle {
            return;
        }
        self.keep_alive_timeout = false;
        self.queue_request_link_status(self.config.remote_addr);
        self.events.push(LinkEvent::KeepAliveInitiated);
        self.start_response_timer(now);
        self.pri_state = PriState::RequestLinkStatusWait;
    }

    // ---- secondary station state machine ----

    fn sec_on_tx_ready(&mut self) {
        match self.sec_state {
            SecState::TransmitWaitReset => self.sec_state = SecState::Reset,
            SecState::TransmitWaitNotReset => self.sec_state = SecState::NotReset,
            _ => tracing::error!("invalid event for secondary state"),
        }
    }

    fn sec_on_reset_link_states(&mut self, source: u16) {
        match self.sec_state {
            SecState::NotReset | SecState::Reset => {
                self.queue_ack(source);
                self.next_read_fcb = true;
                self.sec_state = SecState::TransmitWaitReset;
            }
            _ => tracing::warn!("ignoring link frame, remote is flooding"),
        }
    }

    fn sec_on_request_link_status(&mut self, source: u16) {
        match self.sec_state {
            SecState::NotReset => {
                self.queue_link_status(source);
                self.sec_state = SecState::TransmitWaitNotReset;
            }
            SecState::Reset => {
                self.queue_link_status(source);
                self.sec_state = SecState::TransmitWaitReset;
            }
            _ => tracing::warn!("ignoring link frame, remote is flooding"),
        }
    }

    fn sec_on_test_link_status(&mut self, source: u16, fcb: bool) {
        match self.sec_state {
            SecState::NotReset => {
                self.statistics.num_unexpected_frame += 1;
                tracing::warn!("TestLinkStatus ignored");
            }
            SecState::Reset => {
                if self.next_read_fcb == fcb {
                    self.queue_ack(source);
                    self.next_read_fcb = !self.next_read_fcb;
                    self.sec_state = SecState::TransmitWaitReset;
                } else {
                    tracing::warn!("received TestLinkStatus with invalid FCB");
                }
            }
            _ => tracing::warn!("ignoring link frame, remote is flooding"),
        }
    }

    fn sec_on_confirmed_user_data(
        &mut self,
        source: u16,
        fcb: bool,
        is_broadcast: bool,
        message: Message,
    ) {
        match self.sec_state {
            SecState::NotReset => {
                self.statistics.num_unexpected_frame += 1;
                tracing::warn!("ConfirmedUserData ignored: secondary not reset");
            }
            SecState::Reset => {
                if !is_broadcast {
                    self.queue_ack(source);
                }
                if self.next_read_fcb == fcb {
                    self.next_read_fcb = !self.next_read_fcb;
                    self.push_data_up(message);
                } else {
                    tracing::warn!("ConfirmedUserData ignored: unexpected FCB");
                }
                self.sec_state = SecState::TransmitWaitReset;
            }
            _ => tracing::warn!("ignoring link frame, remote is flooding"),
        }
    }
}
