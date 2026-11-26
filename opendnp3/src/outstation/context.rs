//! Runtime-agnostic DNP3 Outstation protocol state machine.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::config::OutstationConfig;
use super::database::Database;
use super::events::EventType;
use super::handler::{CommandHandler, OutstationApplication};
use crate::app::{
    AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32, ApduHandler,
    ApduHeader, ApduWriter, AppControlField, AppSeqNum, ClassField, ControlRelayOutputBlock,
    DnpTime, HeaderInfo, IinBit, IinField, Indexed, ParseMode, Range, parse_objects,
};
use crate::clock::MonotonicTime;
use crate::enums::{
    CommandStatus, EventClass, FunctionCode, GroupVariation, OperateType, QualifierCode,
    RestartMode,
};
use crate::link::Addresses;
use crate::message::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    SolicitedConfirmWait,
    UnsolicitedConfirmWait,
    NullUnsolicitedConfirmWait,
}

#[derive(Debug, Clone)]
struct PendingSelect {
    seq: u8,
    expires_at: MonotonicTime,
    objects: Vec<u8>,
}

/// Runtime-agnostic DNP3 Outstation.
///
/// Feed incoming APDUs via [`on_receive`](Self::on_receive), drain outbound
/// APDUs via [`pop_transmit`](Self::pop_transmit), and drive timers via
/// [`next_wakeup`](Self::next_wakeup) + [`on_timeout`](Self::on_timeout).
pub struct Outstation {
    pub config: OutstationConfig,
    pub database: Database,

    addresses: Addresses,
    is_online: bool,
    is_transmitting: bool,
    state: State,

    // IIN state
    device_restart: bool,
    broadcast_received: bool,
    broadcast_confirm: bool,

    // Sequence numbers
    sol_seq: AppSeqNum,
    unsol_seq: AppSeqNum,
    confirm_sol_seq: u8,
    confirm_unsol_seq: u8,

    // Unsolicited state
    completed_null_unsol: bool,
    unsol_enabled: ClassField,

    // Timers
    confirm_deadline: Option<MonotonicTime>,
    unsol_retry_deadline: Option<MonotonicTime>,

    // SBO tracking
    pending_select: Option<PendingSelect>,

    // Outbound queue + last solicited response cache (for duplicate request replay)
    tx_queue: VecDeque<Message>,
    last_request: Option<(u8, Vec<u8>)>,
    last_response: Option<Vec<u8>>,
}

impl Outstation {
    pub fn new(config: OutstationConfig, addresses: Addresses) -> Self {
        let database = Database::new(&config.database, config.event_buffer);
        let unsol_enabled = if config.params.allow_unsolicited {
            config.params.unsol_class_mask
        } else {
            ClassField::NONE
        };
        Self {
            config,
            database,
            addresses,
            is_online: false,
            is_transmitting: false,
            state: State::Idle,
            device_restart: true,
            broadcast_received: false,
            broadcast_confirm: false,
            sol_seq: AppSeqNum::default(),
            unsol_seq: AppSeqNum::default(),
            confirm_sol_seq: 0,
            confirm_unsol_seq: 0,
            completed_null_unsol: false,
            unsol_enabled,
            confirm_deadline: None,
            unsol_retry_deadline: None,
            pending_select: None,
            tx_queue: VecDeque::new(),
            last_request: None,
            last_response: None,
        }
    }

    pub fn is_online(&self) -> bool {
        self.is_online
    }

    pub fn pop_transmit(&mut self) -> Option<Message> {
        let msg = self.tx_queue.pop_front()?;
        self.is_transmitting = true;
        Some(msg)
    }

    pub fn has_pending_transmit(&self) -> bool {
        !self.tx_queue.is_empty()
    }

    pub fn next_wakeup(&self) -> Option<MonotonicTime> {
        match (self.confirm_deadline, self.unsol_retry_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        }
    }

    pub fn on_lower_layer_up(&mut self, now: MonotonicTime, app: &dyn OutstationApplication) {
        if self.is_online {
            return;
        }
        self.is_online = true;
        self.check_unsolicited(now, app);
    }

    pub fn on_lower_layer_down(&mut self) {
        self.is_online = false;
        self.is_transmitting = false;
        self.state = State::Idle;
        self.confirm_deadline = None;
        self.unsol_retry_deadline = None;
        self.tx_queue.clear();
        self.database.events.unselect_all();
        self.database.clear_static_selections();
    }

    pub fn on_tx_ready(&mut self, now: MonotonicTime, app: &dyn OutstationApplication) {
        self.is_transmitting = false;
        self.check_unsolicited(now, app);
    }

    /// Notify the outstation that measurements/events were updated so it can
    /// trigger an unsolicited response if enabled.
    pub fn check_for_task_start(&mut self, now: MonotonicTime, app: &dyn OutstationApplication) {
        self.check_unsolicited(now, app);
    }

    pub fn on_timeout(&mut self, now: MonotonicTime, app: &dyn OutstationApplication) {
        if let Some(deadline) = self.confirm_deadline
            && now >= deadline
        {
            self.confirm_deadline = None;
            match self.state {
                State::SolicitedConfirmWait => {
                    tracing::warn!("solicited confirm timeout");
                    self.database.events.unselect_all();
                    self.database.clear_static_selections();
                    self.state = State::Idle;
                }
                State::UnsolicitedConfirmWait | State::NullUnsolicitedConfirmWait => {
                    tracing::warn!("unsolicited confirm timeout");
                    self.database.events.unselect_all();
                    self.unsol_seq.increment();
                    self.state = State::Idle;
                    self.unsol_retry_deadline = Some(now + self.config.params.unsol_retry_delay);
                }
                State::Idle => {}
            }
        }
        if let Some(deadline) = self.unsol_retry_deadline
            && now >= deadline
        {
            self.unsol_retry_deadline = None;
            self.check_unsolicited(now, app);
        }
    }

    /// Process a received request APDU from the master.
    pub fn on_receive(
        &mut self,
        msg: &Message,
        now: MonotonicTime,
        cmd_handler: &mut dyn CommandHandler,
        app: &mut dyn OutstationApplication,
    ) {
        if !self.is_online {
            return;
        }
        let Some(header) = ApduHeader::parse(&msg.payload) else {
            tracing::warn!("APDU too short");
            return;
        };
        let objects = &msg.payload[ApduHeader::SIZE..];
        let is_broadcast = msg.addresses.is_broadcast();

        if is_broadcast {
            self.broadcast_received = true;
            if msg.addresses.destination == crate::link::LinkBroadcastAddress::ShallConfirm as u16 {
                self.broadcast_confirm = true;
            }
        }

        // Handle CONFIRM first
        if header.function == FunctionCode::Confirm {
            self.handle_confirm(header.control, now, app);
            return;
        }

        if !header.control.is_fir_and_fin() {
            tracing::warn!("ignoring multi-fragment request");
            return;
        }

        // Reply address: respond to sender (if respond_to_any_master or matching remote)
        let reply_addr = Addresses::new(self.addresses.source, msg.addresses.source);

        // Duplicate request check (non-broadcast)
        if !is_broadcast
            && self.state == State::Idle
            && let Some((last_seq, ref last_req)) = self.last_request
            && last_seq == header.control.seq
            && last_req.as_slice() == msg.payload.as_slice()
            && let Some(ref resp) = self.last_response
        {
            self.queue_tx(reply_addr, resp.clone());
            return;
        }

        // New request cancels any pending solicited confirm wait
        if self.state == State::SolicitedConfirmWait {
            self.confirm_deadline = None;
            self.database.events.unselect_all();
            self.database.clear_static_selections();
            self.state = State::Idle;
        }

        self.sol_seq = AppSeqNum::new(header.control.seq);

        let no_reply = is_broadcast
            || matches!(
                header.function,
                FunctionCode::DirectOperateNr
                    | FunctionCode::ImmedFreezeNr
                    | FunctionCode::FreezeClearNr
                    | FunctionCode::FreezeAtTimeNr
            );

        let mut response = match header.function {
            FunctionCode::Read => self.handle_read(objects, now, app),
            FunctionCode::Write => self.handle_write(objects, app),
            FunctionCode::Select => {
                self.handle_select(objects, header.control.seq, now, cmd_handler, app)
            }
            FunctionCode::Operate => {
                self.handle_operate(objects, header.control.seq, now, cmd_handler, app)
            }
            FunctionCode::DirectOperate | FunctionCode::DirectOperateNr => {
                let op_type = if header.function == FunctionCode::DirectOperateNr {
                    OperateType::DirectOperateNoAck
                } else {
                    OperateType::DirectOperate
                };
                self.handle_direct_operate(objects, op_type, cmd_handler, app)
            }
            FunctionCode::EnableUnsolicited => self.handle_unsol_toggle(objects, true, app),
            FunctionCode::DisableUnsolicited => self.handle_unsol_toggle(objects, false, app),
            FunctionCode::AssignClass => self.handle_assign_class(objects, app),
            FunctionCode::ColdRestart => self.handle_restart(true, app),
            FunctionCode::WarmRestart => self.handle_restart(false, app),
            FunctionCode::DelayMeasure => self.handle_delay_measure(app),
            FunctionCode::RecordCurrentTime => self.empty_response(IinField::empty(), app),
            FunctionCode::ImmedFreeze
            | FunctionCode::ImmedFreezeNr
            | FunctionCode::FreezeClear
            | FunctionCode::FreezeClearNr => {
                let clear = matches!(
                    header.function,
                    FunctionCode::FreezeClear | FunctionCode::FreezeClearNr
                );
                self.handle_freeze(objects, clear, app)
            }
            _ => self.empty_response(IinField::from_bit(IinBit::FuncNotSupported), app),
        };

        if !no_reply {
            if self.broadcast_confirm {
                if let Some(ctrl) = response.first_mut() {
                    *ctrl |= 0x20;
                }
                self.state = State::SolicitedConfirmWait;
                self.confirm_sol_seq = self.sol_seq.value();
                self.confirm_deadline = Some(now + self.config.params.sol_confirm_timeout);
            } else if self.broadcast_received {
                self.broadcast_received = false;
            }
            self.last_request = Some((header.control.seq, msg.payload.clone()));
            self.last_response = Some(response.clone());
            self.queue_tx(reply_addr, response);
        }
    }

    // ---- Function handlers ----

    fn handle_confirm(
        &mut self,
        ctrl: AppControlField,
        now: MonotonicTime,
        app: &dyn OutstationApplication,
    ) {
        match self.state {
            State::SolicitedConfirmWait if !ctrl.uns && ctrl.seq == self.confirm_sol_seq => {
                self.confirm_deadline = None;
                self.broadcast_received = false;
                self.broadcast_confirm = false;
                self.database.events.clear_written();
                if self.database.events.has_any_selected() || self.database.has_static_selections()
                {
                    self.sol_seq.increment();
                    let resp = self.build_read_fragment(false, now, app);
                    self.queue_tx(self.addresses, resp);
                } else {
                    self.state = State::Idle;
                    self.check_unsolicited(now, app);
                }
            }
            State::NullUnsolicitedConfirmWait if ctrl.uns && ctrl.seq == self.confirm_unsol_seq => {
                self.confirm_deadline = None;
                self.completed_null_unsol = true;
                self.unsol_seq.increment();
                self.state = State::Idle;
                self.check_unsolicited(now, app);
            }
            State::UnsolicitedConfirmWait if ctrl.uns && ctrl.seq == self.confirm_unsol_seq => {
                self.confirm_deadline = None;
                self.broadcast_received = false;
                self.broadcast_confirm = false;
                self.database.events.clear_written();
                self.unsol_seq.increment();
                self.state = State::Idle;
                self.check_unsolicited(now, app);
            }
            _ => {
                tracing::debug!(?ctrl, state = ?self.state, "ignoring unexpected CONFIRM");
            }
        }
    }

    fn handle_read(
        &mut self,
        objects: &[u8],
        now: MonotonicTime,
        app: &dyn OutstationApplication,
    ) -> Vec<u8> {
        self.database.events.unselect_all();
        self.database.clear_static_selections();

        let mut handler = ReadRequestHandler {
            db: &mut self.database,
        };
        let (res, mut req_iin) = parse_objects(objects, ParseMode::HeaderOnly, &mut handler);
        if !res.is_ok() {
            req_iin |= res.to_iin();
        }

        let mut frag = self.build_read_fragment(true, now, app);
        // Merge request-error IINs into response header
        if frag.len() >= 4 {
            frag[2] |= req_iin.lsb;
            frag[3] |= req_iin.msb;
        }
        frag
    }

    fn build_read_fragment(
        &mut self,
        fir: bool,
        now: MonotonicTime,
        app: &dyn OutstationApplication,
    ) -> Vec<u8> {
        let mut writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(fir, true, false, false, self.sol_seq.value()),
            FunctionCode::Response,
            IinField::empty(),
        );

        let events_done = self.database.events.write_selected(&mut writer);
        let static_done = if events_done {
            self.database.write_static(&mut writer)
        } else {
            !self.database.has_static_selections()
        };

        let fin = events_done && static_done;
        let con = !fin || self.database.events.has_any_written();

        writer.set_control(AppControlField::new(
            fir,
            fin,
            con,
            false,
            self.sol_seq.value(),
        ));
        writer.set_iin(self.dynamic_iin(app));

        if con {
            self.state = State::SolicitedConfirmWait;
            self.confirm_sol_seq = self.sol_seq.value();
            self.confirm_deadline = Some(now + self.config.params.sol_confirm_timeout);
        } else {
            self.state = State::Idle;
        }

        writer.into_bytes()
    }

    fn handle_write(&mut self, objects: &[u8], app: &mut dyn OutstationApplication) -> Vec<u8> {
        let mut handler = WriteRequestHandler {
            device_restart: &mut self.device_restart,
            app,
        };
        let (res, mut iin) = parse_objects(objects, ParseMode::WithObjects, &mut handler);
        if !res.is_ok() {
            iin |= res.to_iin();
        }
        self.empty_response(iin, app)
    }

    fn handle_select(
        &mut self,
        objects: &[u8],
        seq: u8,
        now: MonotonicTime,
        cmd_handler: &mut dyn CommandHandler,
        app: &dyn OutstationApplication,
    ) -> Vec<u8> {
        let mut writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(true, true, false, false, self.sol_seq.value()),
            FunctionCode::Response,
            self.dynamic_iin(app),
        );
        cmd_handler.begin();
        let mut handler = CommandExecHandler {
            mode: CmdMode::Select,
            max_controls: self.config.params.max_controls_per_request,
            count: 0,
            writer: &mut writer,
            cmd_handler,
        };
        let (res, mut iin) = parse_objects(objects, ParseMode::WithObjects, &mut handler);
        cmd_handler.end();
        if !res.is_ok() {
            iin |= res.to_iin();
        } else {
            self.pending_select = Some(PendingSelect {
                seq,
                expires_at: now + self.config.params.select_timeout,
                objects: objects.to_vec(),
            });
        }
        writer.set_iin(self.dynamic_iin(app) | iin);
        writer.into_bytes()
    }

    fn handle_operate(
        &mut self,
        objects: &[u8],
        seq: u8,
        now: MonotonicTime,
        cmd_handler: &mut dyn CommandHandler,
        app: &dyn OutstationApplication,
    ) -> Vec<u8> {
        let mode = match self.pending_select.take() {
            Some(sel)
                if seq == AppSeqNum::new(sel.seq).next().value()
                    && sel.objects.as_slice() == objects =>
            {
                if now <= sel.expires_at {
                    CmdMode::Operate(OperateType::SelectBeforeOperate)
                } else {
                    CmdMode::ForceStatus(CommandStatus::Timeout)
                }
            }
            _ => CmdMode::ForceStatus(CommandStatus::NoSelect),
        };

        let mut writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(true, true, false, false, self.sol_seq.value()),
            FunctionCode::Response,
            self.dynamic_iin(app),
        );
        cmd_handler.begin();
        let mut handler = CommandExecHandler {
            mode,
            max_controls: self.config.params.max_controls_per_request,
            count: 0,
            writer: &mut writer,
            cmd_handler,
        };
        let (res, mut iin) = parse_objects(objects, ParseMode::WithObjects, &mut handler);
        cmd_handler.end();
        if !res.is_ok() {
            iin |= res.to_iin();
        }
        writer.set_iin(self.dynamic_iin(app) | iin);
        writer.into_bytes()
    }

    fn handle_direct_operate(
        &mut self,
        objects: &[u8],
        op_type: OperateType,
        cmd_handler: &mut dyn CommandHandler,
        app: &dyn OutstationApplication,
    ) -> Vec<u8> {
        let mut writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(true, true, false, false, self.sol_seq.value()),
            FunctionCode::Response,
            self.dynamic_iin(app),
        );
        cmd_handler.begin();
        let mut handler = CommandExecHandler {
            mode: CmdMode::Operate(op_type),
            max_controls: self.config.params.max_controls_per_request,
            count: 0,
            writer: &mut writer,
            cmd_handler,
        };
        let (res, mut iin) = parse_objects(objects, ParseMode::WithObjects, &mut handler);
        cmd_handler.end();
        if !res.is_ok() {
            iin |= res.to_iin();
        }
        writer.set_iin(self.dynamic_iin(app) | iin);
        writer.into_bytes()
    }

    fn handle_unsol_toggle(
        &mut self,
        objects: &[u8],
        enable: bool,
        app: &dyn OutstationApplication,
    ) -> Vec<u8> {
        if !self.config.params.allow_unsolicited {
            return self.empty_response(IinField::from_bit(IinBit::FuncNotSupported), app);
        }
        let mut handler = ClassMaskHandler::default();
        let (res, mut iin) = parse_objects(objects, ParseMode::HeaderOnly, &mut handler);
        if !res.is_ok() {
            iin |= res.to_iin();
        } else if enable {
            self.unsol_enabled.set(handler.mask);
        } else {
            self.unsol_enabled.clear(handler.mask);
        }
        self.empty_response(iin, app)
    }

    fn handle_assign_class(
        &mut self,
        objects: &[u8],
        app: &mut dyn OutstationApplication,
    ) -> Vec<u8> {
        if !app.supports_assign_class() {
            return self.empty_response(IinField::from_bit(IinBit::FuncNotSupported), app);
        }
        let mut handler = AssignClassHandler {
            db: &mut self.database,
            app,
            current_class: None,
            has_class: false,
        };
        let (res, mut iin) = parse_objects(objects, ParseMode::HeaderOnly, &mut handler);
        if !res.is_ok() {
            iin |= res.to_iin();
        }
        self.empty_response(iin, app)
    }

    fn handle_restart(&mut self, cold: bool, app: &mut dyn OutstationApplication) -> Vec<u8> {
        let mode = if cold {
            app.cold_restart_support()
        } else {
            app.warm_restart_support()
        };
        if mode == RestartMode::Unsupported {
            return self.empty_response(IinField::from_bit(IinBit::FuncNotSupported), app);
        }
        let delay = if cold {
            app.cold_restart()
        } else {
            app.warm_restart()
        };
        let mut writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(true, true, false, false, self.sol_seq.value()),
            FunctionCode::Response,
            self.dynamic_iin(app),
        );
        writer.write_time_delay(mode == RestartMode::SupportedDelayFine, delay);
        writer.into_bytes()
    }

    fn handle_delay_measure(&mut self, app: &dyn OutstationApplication) -> Vec<u8> {
        let mut writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(true, true, false, false, self.sol_seq.value()),
            FunctionCode::Response,
            self.dynamic_iin(app),
        );
        writer.write_time_delay(true, 0);
        writer.into_bytes()
    }

    fn handle_freeze(
        &mut self,
        objects: &[u8],
        clear: bool,
        app: &dyn OutstationApplication,
    ) -> Vec<u8> {
        let now_time = app.now_dnp_time();
        let mut handler = FreezeHandler {
            db: &mut self.database,
            clear,
            time: now_time,
        };
        let (res, mut iin) = parse_objects(objects, ParseMode::HeaderOnly, &mut handler);
        if !res.is_ok() {
            iin |= res.to_iin();
        }
        self.empty_response(iin, app)
    }

    // ---- Unsolicited engine ----

    fn check_unsolicited(&mut self, now: MonotonicTime, app: &dyn OutstationApplication) {
        if !self.is_online
            || !self.config.params.allow_unsolicited
            || self.state != State::Idle
            || self.is_transmitting
            || self.unsol_retry_deadline.is_some()
        {
            return;
        }

        if !self.completed_null_unsol {
            let writer = ApduWriter::new_response(
                self.config.params.max_tx_frag_size,
                AppControlField::new(true, true, true, true, self.unsol_seq.value()),
                FunctionCode::UnsolicitedResponse,
                self.dynamic_iin(app),
            );
            self.state = State::NullUnsolicitedConfirmWait;
            self.confirm_unsol_seq = self.unsol_seq.value();
            self.confirm_deadline = Some(now + self.config.params.unsol_confirm_timeout);
            self.queue_tx(self.addresses, writer.into_bytes());
            return;
        }

        // Check if any enabled unsolicited event class has unwritten events
        let available = self.database.events.unwritten_classes();
        let matching = available.intersect(self.unsol_enabled);
        if !matching.has_event_class() {
            return;
        }

        self.database.events.unselect_all();
        if matching.has_class1() {
            self.database
                .events
                .select_by_class(EventClass::Ec1, usize::MAX);
        }
        if matching.has_class2() {
            self.database
                .events
                .select_by_class(EventClass::Ec2, usize::MAX);
        }
        if matching.has_class3() {
            self.database
                .events
                .select_by_class(EventClass::Ec3, usize::MAX);
        }

        let mut writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(true, true, true, true, self.unsol_seq.value()),
            FunctionCode::UnsolicitedResponse,
            IinField::empty(),
        );
        self.database.events.write_selected(&mut writer);
        writer.set_iin(self.dynamic_iin(app));

        self.state = State::UnsolicitedConfirmWait;
        self.confirm_unsol_seq = self.unsol_seq.value();
        self.confirm_deadline = Some(now + self.config.params.unsol_confirm_timeout);
        if !self.broadcast_confirm {
            self.broadcast_received = false;
        }
        self.queue_tx(self.addresses, writer.into_bytes());
    }

    // ---- Helpers ----

    fn dynamic_iin(&self, app: &dyn OutstationApplication) -> IinField {
        let mut iin = app.application_iin();
        iin.set_bit_to(IinBit::DeviceRestart, self.device_restart);
        iin.set_bit_to(IinBit::AllStations, self.broadcast_received);
        iin.set_bit_to(
            IinBit::EventBufferOverflow,
            self.database.events.is_overflown(),
        );
        let classes = self.database.events.unwritten_classes();
        iin.set_bit_to(IinBit::Class1Events, classes.has_class1());
        iin.set_bit_to(IinBit::Class2Events, classes.has_class2());
        iin.set_bit_to(IinBit::Class3Events, classes.has_class3());
        iin
    }

    fn empty_response(&self, extra_iin: IinField, app: &dyn OutstationApplication) -> Vec<u8> {
        let writer = ApduWriter::new_response(
            self.config.params.max_tx_frag_size,
            AppControlField::new(true, true, false, false, self.sol_seq.value()),
            FunctionCode::Response,
            self.dynamic_iin(app) | extra_iin,
        );
        writer.into_bytes()
    }

    fn queue_tx(&mut self, addr: Addresses, payload: Vec<u8>) {
        self.tx_queue.push_back(Message::new(addr, payload));
    }
}

// =========================================================================
// Internal APDU handlers
// =========================================================================

struct ReadRequestHandler<'a> {
    db: &'a mut Database,
}

impl ApduHandler for ReadRequestHandler<'_> {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        match info.gv {
            GroupVariation::Group60Var1 => {
                self.db.select_class0();
                IinField::empty()
            }
            GroupVariation::Group60Var2 => {
                self.db.events.select_by_class(EventClass::Ec1, usize::MAX);
                IinField::empty()
            }
            GroupVariation::Group60Var3 => {
                self.db.events.select_by_class(EventClass::Ec2, usize::MAX);
                IinField::empty()
            }
            GroupVariation::Group60Var4 => {
                self.db.events.select_by_class(EventClass::Ec3, usize::MAX);
                IinField::empty()
            }
            _ => {
                if let Some(ty) = gv_to_event_type(info.gv) {
                    self.db.events.select_by_type(ty, info.gv, usize::MAX);
                    IinField::empty()
                } else if self.db.select_static(info.gv, None) {
                    IinField::empty()
                } else {
                    IinField::from_bit(IinBit::ObjectUnknown)
                }
            }
        }
    }

    fn on_range_header(&mut self, info: HeaderInfo, range: Range) -> IinField {
        if self.db.select_static(info.gv, Some(range)) {
            IinField::empty()
        } else {
            IinField::from_bit(IinBit::ParamError)
        }
    }

    fn on_count_header(&mut self, info: HeaderInfo, count: u16) -> IinField {
        let limit = count as usize;
        match info.gv {
            GroupVariation::Group60Var2 => {
                self.db.events.select_by_class(EventClass::Ec1, limit);
                IinField::empty()
            }
            GroupVariation::Group60Var3 => {
                self.db.events.select_by_class(EventClass::Ec2, limit);
                IinField::empty()
            }
            GroupVariation::Group60Var4 => {
                self.db.events.select_by_class(EventClass::Ec3, limit);
                IinField::empty()
            }
            _ => {
                if let Some(ty) = gv_to_event_type(info.gv) {
                    self.db.events.select_by_type(ty, info.gv, limit);
                    IinField::empty()
                } else {
                    IinField::from_bit(IinBit::ObjectUnknown)
                }
            }
        }
    }
}

fn gv_to_event_type(gv: GroupVariation) -> Option<EventType> {
    match gv.group() {
        2 => Some(EventType::Binary),
        4 => Some(EventType::DoubleBitBinary),
        11 => Some(EventType::BinaryOutputStatus),
        22 => Some(EventType::Counter),
        23 => Some(EventType::FrozenCounter),
        32 => Some(EventType::Analog),
        42 => Some(EventType::AnalogOutputStatus),
        111 => Some(EventType::OctetString),
        _ => None,
    }
}

struct WriteRequestHandler<'a> {
    device_restart: &'a mut bool,
    app: &'a mut dyn OutstationApplication,
}

impl ApduHandler for WriteRequestHandler<'_> {
    fn on_iin_bits(&mut self, _info: HeaderInfo, values: &[Indexed<bool>]) -> IinField {
        let mut iin = IinField::empty();
        for item in values {
            if item.index == IinBit::DeviceRestart as u16 && !item.value {
                *self.device_restart = false;
            } else {
                iin.set_bit(IinBit::ParamError);
            }
        }
        iin
    }

    fn on_time_g50v1(&mut self, _info: HeaderInfo, values: &[DnpTime]) -> IinField {
        if !self.app.supports_write_absolute_time() {
            return IinField::from_bit(IinBit::ParamError);
        }
        if values.len() != 1 {
            return IinField::from_bit(IinBit::ParamError);
        }
        if self.app.write_absolute_time(values[0]) {
            IinField::empty()
        } else {
            IinField::from_bit(IinBit::ParamError)
        }
    }
}

#[derive(Clone, Copy)]
enum CmdMode {
    Select,
    Operate(OperateType),
    ForceStatus(CommandStatus),
}

struct CommandExecHandler<'a> {
    mode: CmdMode,
    max_controls: usize,
    count: usize,
    writer: &'a mut ApduWriter,
    cmd_handler: &'a mut dyn CommandHandler,
}

impl CommandExecHandler<'_> {
    fn next_status(
        &mut self,
        f: impl FnOnce(CmdMode, &mut dyn CommandHandler) -> CommandStatus,
    ) -> CommandStatus {
        self.count += 1;
        if self.count > self.max_controls {
            return CommandStatus::TooManyOps;
        }
        let mode = self.mode;
        match mode {
            CmdMode::ForceStatus(st) => st,
            _ => f(mode, self.cmd_handler),
        }
    }
}

impl ApduHandler for CommandExecHandler<'_> {
    fn on_crobs(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<ControlRelayOutputBlock>],
    ) -> IinField {
        let q = info.qualifier;
        let use_u8 = q == QualifierCode::Uint8CntUint8Index;
        self.writer.write_bytes(&[12, 1, q.to_u8()]);
        if use_u8 {
            self.writer.write_u8(values.len() as u8);
        } else {
            self.writer.write_u16(values.len() as u16);
        }
        let mut iin = IinField::empty();
        for item in values {
            let status = if item.value.is_qu_bit_set() {
                CommandStatus::NotSupported
            } else {
                self.next_status(|mode, h| match mode {
                    CmdMode::Select => h.select_crob(&item.value, item.index),
                    CmdMode::Operate(op) => h.operate_crob(&item.value, item.index, op),
                    CmdMode::ForceStatus(st) => st,
                })
            };
            if status == CommandStatus::NotSupported {
                iin.set_bit(IinBit::ParamError);
            }
            let mut echoed = item.value;
            echoed.status = status;
            if use_u8 {
                self.writer.write_u8(item.index as u8);
            } else {
                self.writer.write_u16(item.index);
            }
            self.writer.write_crob(&echoed);
        }
        iin
    }

    fn on_ao_i32s(&mut self, info: HeaderInfo, values: &[Indexed<AnalogOutputInt32>]) -> IinField {
        let q = info.qualifier;
        let use_u8 = q == QualifierCode::Uint8CntUint8Index;
        self.writer.write_bytes(&[41, 1, q.to_u8()]);
        if use_u8 {
            self.writer.write_u8(values.len() as u8);
        } else {
            self.writer.write_u16(values.len() as u16);
        }
        for item in values {
            let status = self.next_status(|mode, h| match mode {
                CmdMode::Select => h.select_ao_i32(&item.value, item.index),
                CmdMode::Operate(op) => h.operate_ao_i32(&item.value, item.index, op),
                CmdMode::ForceStatus(st) => st,
            });
            let echoed = AnalogOutputInt32::with_status(item.value.value, status);
            if use_u8 {
                self.writer.write_u8(item.index as u8);
            } else {
                self.writer.write_u16(item.index);
            }
            self.writer.write_ao_i32(&echoed);
        }
        IinField::empty()
    }

    fn on_ao_i16s(&mut self, info: HeaderInfo, values: &[Indexed<AnalogOutputInt16>]) -> IinField {
        let q = info.qualifier;
        let use_u8 = q == QualifierCode::Uint8CntUint8Index;
        self.writer.write_bytes(&[41, 2, q.to_u8()]);
        if use_u8 {
            self.writer.write_u8(values.len() as u8);
        } else {
            self.writer.write_u16(values.len() as u16);
        }
        for item in values {
            let status = self.next_status(|mode, h| match mode {
                CmdMode::Select => h.select_ao_i16(&item.value, item.index),
                CmdMode::Operate(op) => h.operate_ao_i16(&item.value, item.index, op),
                CmdMode::ForceStatus(st) => st,
            });
            let echoed = AnalogOutputInt16::with_status(item.value.value, status);
            if use_u8 {
                self.writer.write_u8(item.index as u8);
            } else {
                self.writer.write_u16(item.index);
            }
            self.writer.write_ao_i16(&echoed);
        }
        IinField::empty()
    }

    fn on_ao_f32s(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputFloat32>],
    ) -> IinField {
        let q = info.qualifier;
        let use_u8 = q == QualifierCode::Uint8CntUint8Index;
        self.writer.write_bytes(&[41, 3, q.to_u8()]);
        if use_u8 {
            self.writer.write_u8(values.len() as u8);
        } else {
            self.writer.write_u16(values.len() as u16);
        }
        for item in values {
            let status = self.next_status(|mode, h| match mode {
                CmdMode::Select => h.select_ao_f32(&item.value, item.index),
                CmdMode::Operate(op) => h.operate_ao_f32(&item.value, item.index, op),
                CmdMode::ForceStatus(st) => st,
            });
            let echoed = AnalogOutputFloat32::with_status(item.value.value, status);
            if use_u8 {
                self.writer.write_u8(item.index as u8);
            } else {
                self.writer.write_u16(item.index);
            }
            self.writer.write_ao_f32(&echoed);
        }
        IinField::empty()
    }

    fn on_ao_f64s(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputDouble64>],
    ) -> IinField {
        let q = info.qualifier;
        let use_u8 = q == QualifierCode::Uint8CntUint8Index;
        self.writer.write_bytes(&[41, 4, q.to_u8()]);
        if use_u8 {
            self.writer.write_u8(values.len() as u8);
        } else {
            self.writer.write_u16(values.len() as u16);
        }
        for item in values {
            let status = self.next_status(|mode, h| match mode {
                CmdMode::Select => h.select_ao_f64(&item.value, item.index),
                CmdMode::Operate(op) => h.operate_ao_f64(&item.value, item.index, op),
                CmdMode::ForceStatus(st) => st,
            });
            let echoed = AnalogOutputDouble64::with_status(item.value.value, status);
            if use_u8 {
                self.writer.write_u8(item.index as u8);
            } else {
                self.writer.write_u16(item.index);
            }
            self.writer.write_ao_f64(&echoed);
        }
        IinField::empty()
    }
}

#[derive(Default)]
struct ClassMaskHandler {
    mask: ClassField,
}

impl ApduHandler for ClassMaskHandler {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        match info.gv {
            GroupVariation::Group60Var2 => {
                self.mask.set(ClassField::CLASS_1);
                IinField::empty()
            }
            GroupVariation::Group60Var3 => {
                self.mask.set(ClassField::CLASS_2);
                IinField::empty()
            }
            GroupVariation::Group60Var4 => {
                self.mask.set(ClassField::CLASS_3);
                IinField::empty()
            }
            _ => IinField::from_bit(IinBit::ObjectUnknown),
        }
    }
}

struct AssignClassHandler<'a> {
    db: &'a mut Database,
    app: &'a mut dyn OutstationApplication,
    current_class: Option<EventClass>,
    has_class: bool,
}

impl ApduHandler for AssignClassHandler<'_> {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        match info.gv {
            GroupVariation::Group60Var1 => {
                self.current_class = None;
                self.has_class = true;
                IinField::empty()
            }
            GroupVariation::Group60Var2 => {
                self.current_class = Some(EventClass::Ec1);
                self.has_class = true;
                IinField::empty()
            }
            GroupVariation::Group60Var3 => {
                self.current_class = Some(EventClass::Ec2);
                self.has_class = true;
                IinField::empty()
            }
            GroupVariation::Group60Var4 => {
                self.current_class = Some(EventClass::Ec3);
                self.has_class = true;
                IinField::empty()
            }
            _ => {
                if !self.has_class {
                    return IinField::from_bit(IinBit::ParamError);
                }
                let updated = self.db.assign_class(info.gv, None, self.current_class);
                if updated == 0 {
                    IinField::from_bit(IinBit::ParamError)
                } else {
                    self.app.record_class_assignment(
                        info.gv.group(),
                        self.current_class,
                        Range::new(0, (updated - 1) as u16),
                    );
                    IinField::empty()
                }
            }
        }
    }

    fn on_range_header(&mut self, info: HeaderInfo, range: Range) -> IinField {
        if !self.has_class {
            return IinField::from_bit(IinBit::ParamError);
        }
        let expected = usize::from(range.stop) - usize::from(range.start) + 1;
        let updated = self
            .db
            .assign_class(info.gv, Some(range), self.current_class);
        if updated > 0 {
            self.app
                .record_class_assignment(info.gv.group(), self.current_class, range);
        }
        if updated != expected {
            IinField::from_bit(IinBit::ParamError)
        } else {
            IinField::empty()
        }
    }
}

struct FreezeHandler<'a> {
    db: &'a mut Database,
    clear: bool,
    time: DnpTime,
}

impl ApduHandler for FreezeHandler<'_> {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        if info.group == 20 {
            self.db.freeze_counters(None, self.clear, self.time);
            IinField::empty()
        } else {
            IinField::from_bit(IinBit::ObjectUnknown)
        }
    }

    fn on_range_header(&mut self, info: HeaderInfo, range: Range) -> IinField {
        if info.group == 20 {
            self.db.freeze_counters(Some(range), self.clear, self.time);
            IinField::empty()
        } else {
            IinField::from_bit(IinBit::ObjectUnknown)
        }
    }
}
