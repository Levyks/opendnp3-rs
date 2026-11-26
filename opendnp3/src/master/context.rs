//! Runtime-agnostic DNP3 Master protocol state machine.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::command::CommandSet;
use super::config::MasterParams;
use super::handler::{
    CommandPointResult, CommandResult, MasterApplication, ResponseInfo, SoeHandler,
};
use crate::app::{
    Analog, AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    AnalogOutputStatus, ApduHandler, ApduHeader, ApduResponseHeader, ApduWriter, AppControlField,
    AppSeqNum, Binary, BinaryOutputStatus, ClassField, ControlRelayOutputBlock, Counter, DnpTime,
    DoubleBitBinary, FrozenCounter, HeaderInfo, IinBit, IinField, Indexed, OctetString, ParseMode,
    Range, TimeAndInterval, parse_objects,
};
use crate::enums::{
    CommandPointState, CommandStatus, FunctionCode, GroupVariation, MasterTaskType, PointClass,
    RestartType, TaskCompletion, TimeSyncMode,
};
use crate::link::Addresses;
use crate::message::Message;

/// Events emitted by the master for the application to inspect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterEvent {
    TaskComplete {
        task_type: MasterTaskType,
        result: TaskCompletion,
    },
    CommandComplete(CommandResult),
    RestartComplete {
        result: TaskCompletion,
        delay_ms: Option<u64>,
    },
}

#[derive(Debug, Clone)]
enum ScanSpec {
    Classes(ClassField),
    Range(GroupVariation, Range),
    AllObjects(GroupVariation),
}

#[derive(Debug, Clone)]
struct PeriodicScan {
    spec: ScanSpec,
    period: Duration,
    next_run: Option<Instant>,
}

#[derive(Debug, Clone)]
enum QueuedTask {
    ClearRestart,
    DisableUnsol(ClassField),
    EnableUnsol(ClassField),
    IntegrityPoll(ClassField),
    EventScan(ClassField),
    RangeScan(GroupVariation, Range),
    AllObjectsScan(GroupVariation),
    AssignClass {
        class: PointClass,
        gv: GroupVariation,
        range: Option<Range>,
    },
    NonLanTimeSync {
        measured: bool,
        start_time: Option<Instant>,
    },
    LanTimeSync {
        recorded: bool,
        recorded_time: DnpTime,
    },
    SelectAndOperate {
        commands: CommandSet,
        selected: bool,
        select_payload: Vec<u8>,
    },
    DirectOperate {
        commands: CommandSet,
        no_ack: bool,
    },
    Restart(RestartType),
}

impl QueuedTask {
    fn task_type(&self) -> MasterTaskType {
        match self {
            Self::ClearRestart => MasterTaskType::ClearRestart,
            Self::DisableUnsol(_) => MasterTaskType::DisableUnsol,
            Self::EnableUnsol(_) => MasterTaskType::EnableUnsol,
            Self::IntegrityPoll(_) => MasterTaskType::StartupIntegrityPoll,
            Self::EventScan(_) => MasterTaskType::AutoEventScan,
            Self::AssignClass { .. } => MasterTaskType::AssignClass,
            Self::NonLanTimeSync { .. } => MasterTaskType::NonLanTimeSync,
            Self::LanTimeSync { .. } => MasterTaskType::LanTimeSync,
            Self::RangeScan(..)
            | Self::AllObjectsScan(_)
            | Self::SelectAndOperate { .. }
            | Self::DirectOperate { .. }
            | Self::Restart(_) => MasterTaskType::UserTask,
        }
    }
}

/// Runtime-agnostic DNP3 Master.
pub struct Master {
    pub params: MasterParams,
    addresses: Addresses,
    is_online: bool,
    is_transmitting: bool,

    sol_seq: AppSeqNum,
    active_task: Option<QueuedTask>,
    task_queue: VecDeque<QueuedTask>,
    periodic_scans: Vec<PeriodicScan>,
    response_deadline: Option<Instant>,

    tx_queue: VecDeque<Message>,
    events: Vec<MasterEvent>,
}

impl Master {
    pub fn new(params: MasterParams, addresses: Addresses) -> Self {
        Self {
            params,
            addresses,
            is_online: false,
            is_transmitting: false,
            sol_seq: AppSeqNum::default(),
            active_task: None,
            task_queue: VecDeque::new(),
            periodic_scans: Vec::new(),
            response_deadline: None,
            tx_queue: VecDeque::new(),
            events: Vec::new(),
        }
    }

    pub fn is_online(&self) -> bool {
        self.is_online
    }

    pub fn drain_events(&mut self) -> Vec<MasterEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn pop_transmit(&mut self) -> Option<Message> {
        let msg = self.tx_queue.pop_front()?;
        self.is_transmitting = true;
        Some(msg)
    }

    pub fn has_pending_transmit(&self) -> bool {
        !self.tx_queue.is_empty()
    }

    pub fn next_wakeup(&self) -> Option<Instant> {
        let mut earliest = self.response_deadline;
        if self.is_online {
            for scan in &self.periodic_scans {
                if let Some(t) = scan.next_run {
                    earliest = Some(match earliest {
                        Some(cur) => cur.min(t),
                        None => t,
                    });
                }
            }
        }
        earliest
    }

    // ---- Periodic scans ----

    pub fn add_class_scan(&mut self, classes: ClassField, period: Duration, now: Instant) {
        self.periodic_scans.push(PeriodicScan {
            spec: ScanSpec::Classes(classes),
            period,
            next_run: if self.is_online {
                Some(now + period)
            } else {
                None
            },
        });
    }

    pub fn add_range_scan(
        &mut self,
        gv: GroupVariation,
        range: Range,
        period: Duration,
        now: Instant,
    ) {
        self.periodic_scans.push(PeriodicScan {
            spec: ScanSpec::Range(gv, range),
            period,
            next_run: if self.is_online {
                Some(now + period)
            } else {
                None
            },
        });
    }

    pub fn add_all_objects_scan(&mut self, gv: GroupVariation, period: Duration, now: Instant) {
        self.periodic_scans.push(PeriodicScan {
            spec: ScanSpec::AllObjects(gv),
            period,
            next_run: if self.is_online {
                Some(now + period)
            } else {
                None
            },
        });
    }

    // ---- Lifecycle ----

    pub fn on_lower_layer_up(&mut self, now: Instant, app: &mut dyn MasterApplication) {
        if self.is_online {
            return;
        }
        self.is_online = true;
        for scan in &mut self.periodic_scans {
            scan.next_run = Some(now + scan.period);
        }
        self.queue_startup_tasks();
        self.try_start_next_task(now, app);
    }

    pub fn on_lower_layer_down(&mut self, app: &mut dyn MasterApplication) {
        self.is_online = false;
        self.is_transmitting = false;
        self.response_deadline = None;
        self.tx_queue.clear();
        for scan in &mut self.periodic_scans {
            scan.next_run = None;
        }

        if let Some(task) = self.active_task.take() {
            self.finish_task_failure(task, TaskCompletion::FailureNoComms, app);
        }
        while let Some(task) = self.task_queue.pop_front() {
            self.finish_task_failure(task, TaskCompletion::FailureNoComms, app);
        }
    }

    pub fn on_tx_ready(&mut self, now: Instant, app: &mut dyn MasterApplication) {
        self.is_transmitting = false;
        // If active task is DirectOperateNoAck, complete immediately upon TX
        if matches!(
            self.active_task,
            Some(QueuedTask::DirectOperate { no_ack: true, .. })
        ) && let Some(task) = self.active_task.take()
        {
            self.response_deadline = None;
            self.events
                .push(MasterEvent::CommandComplete(CommandResult {
                    summary: TaskCompletion::Success,
                    points: Vec::new(),
                }));
            app.on_task_complete(task.task_type(), TaskCompletion::Success);
        }
        self.try_start_next_task(now, app);
    }

    pub fn on_timeout(&mut self, now: Instant, app: &mut dyn MasterApplication) {
        if let Some(deadline) = self.response_deadline
            && now >= deadline
        {
            self.response_deadline = None;
            if let Some(task) = self.active_task.take() {
                tracing::warn!(task = ?task.task_type(), "master response timeout");
                self.finish_task_failure(task, TaskCompletion::FailureResponseTimeout, app);
            }
        }
        if self.is_online {
            for scan in &mut self.periodic_scans {
                if let Some(due) = scan.next_run
                    && now >= due
                {
                    scan.next_run = Some(now + scan.period);
                    match scan.spec {
                        ScanSpec::Classes(c) => {
                            if c.has_class0() {
                                self.task_queue.push_back(QueuedTask::IntegrityPoll(c));
                            } else if c.has_event_class() {
                                self.task_queue.push_back(QueuedTask::EventScan(c));
                            }
                        }
                        ScanSpec::Range(gv, r) => {
                            self.task_queue.push_back(QueuedTask::RangeScan(gv, r));
                        }
                        ScanSpec::AllObjects(gv) => {
                            self.task_queue.push_back(QueuedTask::AllObjectsScan(gv));
                        }
                    }
                }
            }
        }
        self.try_start_next_task(now, app);
    }

    pub fn assign_class_all(
        &mut self,
        class: PointClass,
        gv: GroupVariation,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue.push_back(QueuedTask::AssignClass {
            class,
            gv,
            range: None,
        });
        self.try_start_next_task(now, app);
    }

    pub fn assign_class_range(
        &mut self,
        class: PointClass,
        gv: GroupVariation,
        range: Range,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue.push_back(QueuedTask::AssignClass {
            class,
            gv,
            range: Some(range),
        });
        self.try_start_next_task(now, app);
    }

    // ---- User API ----

    pub fn scan_classes(
        &mut self,
        classes: ClassField,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        if !self.is_online {
            app.on_task_complete(MasterTaskType::UserTask, TaskCompletion::FailureNoComms);
            return;
        }
        if classes.has_class0() {
            self.task_queue
                .push_back(QueuedTask::IntegrityPoll(classes));
        } else if classes.has_event_class() {
            self.task_queue.push_back(QueuedTask::EventScan(classes));
        }
        self.try_start_next_task(now, app);
    }

    pub fn scan_range(
        &mut self,
        gv: GroupVariation,
        range: Range,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue.push_back(QueuedTask::RangeScan(gv, range));
        self.try_start_next_task(now, app);
    }

    pub fn scan_all_objects(
        &mut self,
        gv: GroupVariation,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue.push_back(QueuedTask::AllObjectsScan(gv));
        self.try_start_next_task(now, app);
    }

    pub fn select_and_operate(
        &mut self,
        commands: CommandSet,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        if !self.is_online {
            app.on_task_complete(MasterTaskType::UserTask, TaskCompletion::FailureNoComms);
            return;
        }
        self.task_queue.push_back(QueuedTask::SelectAndOperate {
            commands,
            selected: false,
            select_payload: Vec::new(),
        });
        self.try_start_next_task(now, app);
    }

    pub fn direct_operate(
        &mut self,
        commands: CommandSet,
        no_ack: bool,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue
            .push_back(QueuedTask::DirectOperate { commands, no_ack });
        self.try_start_next_task(now, app);
    }

    pub fn restart(
        &mut self,
        restart_type: RestartType,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue.push_back(QueuedTask::Restart(restart_type));
        self.try_start_next_task(now, app);
    }

    pub fn enable_unsolicited(
        &mut self,
        classes: ClassField,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue.push_back(QueuedTask::EnableUnsol(classes));
        self.try_start_next_task(now, app);
    }

    pub fn disable_unsolicited(
        &mut self,
        classes: ClassField,
        now: Instant,
        app: &mut dyn MasterApplication,
    ) {
        self.task_queue.push_back(QueuedTask::DisableUnsol(classes));
        self.try_start_next_task(now, app);
    }

    // ---- Receive path ----

    pub fn on_receive(
        &mut self,
        msg: &Message,
        now: Instant,
        soe: &mut dyn SoeHandler,
        app: &mut dyn MasterApplication,
    ) {
        if !self.is_online {
            return;
        }
        let Some(header) = ApduResponseHeader::parse(&msg.payload) else {
            tracing::warn!("response APDU too short");
            return;
        };
        let objects = &msg.payload[ApduResponseHeader::SIZE..];

        app.on_receive_iin(header.iin);
        self.process_iin_triggers(header.iin);

        match header.function {
            FunctionCode::UnsolicitedResponse => {
                if !header.control.uns || !header.control.is_fir_and_fin() {
                    return;
                }
                if header.control.con {
                    let confirm = ApduHeader::confirm(header.control.seq, true).to_bytes();
                    self.tx_queue
                        .push_back(Message::new(self.addresses, confirm.to_vec()));
                }
                if !objects.is_empty() {
                    self.dispatch_measurements(
                        objects,
                        ResponseInfo {
                            unsolicited: true,
                            fir: header.control.fir,
                            fin: header.control.fin,
                        },
                        soe,
                    );
                }
                self.try_start_next_task(now, app);
            }
            FunctionCode::Response => {
                if header.control.uns {
                    return;
                }
                if header.control.seq != self.sol_seq.value() {
                    tracing::warn!(
                        got = header.control.seq,
                        expected = self.sol_seq.value(),
                        "unexpected solicited response sequence"
                    );
                    return;
                }
                if header.control.con {
                    let confirm = ApduHeader::confirm(header.control.seq, false).to_bytes();
                    self.tx_queue
                        .push_back(Message::new(self.addresses, confirm.to_vec()));
                }

                let Some(mut task) = self.active_task.take() else {
                    return;
                };

                match &mut task {
                    QueuedTask::SelectAndOperate {
                        commands,
                        selected,
                        select_payload,
                    } => {
                        let mut cmd_reader = CommandResponseCollector::default();
                        let (res, _) =
                            parse_objects(objects, ParseMode::WithObjects, &mut cmd_reader);
                        if !res.is_ok() {
                            self.response_deadline = None;
                            self.finish_task_failure(task, TaskCompletion::FailureBadResponse, app);
                        } else if !*selected {
                            // Verify SELECT response matched request and succeeded
                            let all_ok = cmd_reader
                                .points
                                .iter()
                                .all(|p| p.status == CommandStatus::Success)
                                && select_payload.as_slice() == objects;
                            if all_ok && header.control.is_fir_and_fin() {
                                *selected = true;
                                self.sol_seq.increment();
                                let mut w = ApduWriter::new_request(
                                    self.params.max_tx_frag_size,
                                    AppControlField::request(self.sol_seq.value()),
                                    FunctionCode::Operate,
                                );
                                commands.write(&mut w);
                                self.response_deadline = Some(now + self.params.response_timeout);
                                self.tx_queue
                                    .push_back(Message::new(self.addresses, w.into_bytes()));
                                self.active_task = Some(task);
                            } else {
                                self.response_deadline = None;
                                let points = cmd_reader
                                    .points
                                    .into_iter()
                                    .map(|mut p| {
                                        p.state = if p.status != CommandStatus::Success {
                                            CommandPointState::SelectFail
                                        } else {
                                            CommandPointState::SelectMismatch
                                        };
                                        p
                                    })
                                    .collect();
                                self.events
                                    .push(MasterEvent::CommandComplete(CommandResult {
                                        summary: TaskCompletion::Success,
                                        points,
                                    }));
                                app.on_task_complete(
                                    MasterTaskType::UserTask,
                                    TaskCompletion::Success,
                                );
                            }
                        } else {
                            // OPERATE response received
                            self.response_deadline = None;
                            self.events
                                .push(MasterEvent::CommandComplete(CommandResult {
                                    summary: TaskCompletion::Success,
                                    points: cmd_reader.points,
                                }));
                            app.on_task_complete(MasterTaskType::UserTask, TaskCompletion::Success);
                        }
                    }
                    QueuedTask::DirectOperate { .. } => {
                        self.response_deadline = None;
                        let mut cmd_reader = CommandResponseCollector::default();
                        let (res, _) =
                            parse_objects(objects, ParseMode::WithObjects, &mut cmd_reader);
                        let summary = if res.is_ok() {
                            TaskCompletion::Success
                        } else {
                            TaskCompletion::FailureBadResponse
                        };
                        self.events
                            .push(MasterEvent::CommandComplete(CommandResult {
                                summary,
                                points: cmd_reader.points,
                            }));
                        app.on_task_complete(MasterTaskType::UserTask, summary);
                    }
                    QueuedTask::NonLanTimeSync {
                        measured,
                        start_time,
                    } => {
                        if !*measured {
                            let mut delay_reader = DelayCollector::default();
                            let (res, _) =
                                parse_objects(objects, ParseMode::WithObjects, &mut delay_reader);
                            if res.is_ok() && !header.iin.has_request_error() {
                                *measured = true;
                                let rtt_ms = start_time
                                    .map(|s| now.saturating_duration_since(s).as_millis() as u64)
                                    .unwrap_or(0);
                                let turn_ms = delay_reader.delay_ms.unwrap_or(0);
                                let prop_ms = rtt_ms.saturating_sub(turn_ms) / 2;
                                let target_time =
                                    DnpTime::new(app.now_dnp_time().value.saturating_add(prop_ms));
                                self.sol_seq.increment();
                                let mut w = ApduWriter::new_request(
                                    self.params.max_tx_frag_size,
                                    AppControlField::request(self.sol_seq.value()),
                                    FunctionCode::Write,
                                );
                                w.write_time_g50v1(target_time);
                                self.response_deadline = Some(now + self.params.response_timeout);
                                self.tx_queue
                                    .push_back(Message::new(self.addresses, w.into_bytes()));
                                self.active_task = Some(task);
                            } else {
                                self.response_deadline = None;
                                self.finish_task_failure(
                                    task,
                                    TaskCompletion::FailureBadResponse,
                                    app,
                                );
                            }
                        } else {
                            self.response_deadline = None;
                            let res = if header.iin.has_request_error() {
                                TaskCompletion::FailureBadResponse
                            } else {
                                TaskCompletion::Success
                            };
                            self.events.push(MasterEvent::TaskComplete {
                                task_type: MasterTaskType::NonLanTimeSync,
                                result: res,
                            });
                            app.on_task_complete(MasterTaskType::NonLanTimeSync, res);
                        }
                    }
                    QueuedTask::LanTimeSync {
                        recorded,
                        recorded_time,
                    } => {
                        if !*recorded {
                            if !header.iin.has_request_error() {
                                *recorded = true;
                                self.sol_seq.increment();
                                let mut w = ApduWriter::new_request(
                                    self.params.max_tx_frag_size,
                                    AppControlField::request(self.sol_seq.value()),
                                    FunctionCode::Write,
                                );
                                w.write_time_g50v3(*recorded_time);
                                self.response_deadline = Some(now + self.params.response_timeout);
                                self.tx_queue
                                    .push_back(Message::new(self.addresses, w.into_bytes()));
                                self.active_task = Some(task);
                            } else {
                                self.response_deadline = None;
                                self.finish_task_failure(
                                    task,
                                    TaskCompletion::FailureBadResponse,
                                    app,
                                );
                            }
                        } else {
                            self.response_deadline = None;
                            let res = if header.iin.has_request_error() {
                                TaskCompletion::FailureBadResponse
                            } else {
                                TaskCompletion::Success
                            };
                            self.events.push(MasterEvent::TaskComplete {
                                task_type: MasterTaskType::LanTimeSync,
                                result: res,
                            });
                            app.on_task_complete(MasterTaskType::LanTimeSync, res);
                        }
                    }
                    QueuedTask::Restart(_) => {
                        self.response_deadline = None;
                        let mut delay_reader = DelayCollector::default();
                        let (res, _) =
                            parse_objects(objects, ParseMode::WithObjects, &mut delay_reader);
                        let summary = if res.is_ok() && !header.iin.has_request_error() {
                            TaskCompletion::Success
                        } else {
                            TaskCompletion::FailureBadResponse
                        };
                        self.events.push(MasterEvent::RestartComplete {
                            result: summary,
                            delay_ms: delay_reader.delay_ms,
                        });
                        app.on_task_complete(MasterTaskType::UserTask, summary);
                    }
                    _ => {
                        if !objects.is_empty() {
                            self.dispatch_measurements(
                                objects,
                                ResponseInfo {
                                    unsolicited: false,
                                    fir: header.control.fir,
                                    fin: header.control.fin,
                                },
                                soe,
                            );
                        }
                        if header.control.fin {
                            self.response_deadline = None;
                            let tt = task.task_type();
                            self.events.push(MasterEvent::TaskComplete {
                                task_type: tt,
                                result: TaskCompletion::Success,
                            });
                            app.on_task_complete(tt, TaskCompletion::Success);
                        } else {
                            // Multi-fragment response: expect next seq
                            self.sol_seq.increment();
                            self.response_deadline = Some(now + self.params.response_timeout);
                            self.active_task = Some(task);
                        }
                    }
                }

                self.try_start_next_task(now, app);
            }
            _ => {}
        }
    }

    // ---- Internal task engine ----

    fn queue_startup_tasks(&mut self) {
        if self.params.disable_unsol_on_startup && self.params.unsol_class_mask.has_event_class() {
            self.task_queue
                .push_back(QueuedTask::DisableUnsol(self.params.unsol_class_mask));
        }
        if self.params.startup_integrity_class_mask.has_any_class() {
            self.task_queue.push_back(QueuedTask::IntegrityPoll(
                self.params.startup_integrity_class_mask,
            ));
        }
        if self.params.unsol_class_mask.has_event_class() {
            self.task_queue
                .push_back(QueuedTask::EnableUnsol(self.params.unsol_class_mask));
        }
    }

    fn process_iin_triggers(&mut self, iin: IinField) {
        if iin.is_set(IinBit::DeviceRestart)
            && !self.params.ignore_restart_iin
            && !self.has_queued(|t| matches!(t, QueuedTask::ClearRestart))
        {
            self.task_queue.push_front(QueuedTask::ClearRestart);
        }
        if iin.is_set(IinBit::NeedTime) {
            match self.params.time_sync_mode {
                TimeSyncMode::NonLan
                    if !self.has_queued(|t| matches!(t, QueuedTask::NonLanTimeSync { .. })) =>
                {
                    self.task_queue.push_back(QueuedTask::NonLanTimeSync {
                        measured: false,
                        start_time: None,
                    });
                }
                TimeSyncMode::Lan
                    if !self.has_queued(|t| matches!(t, QueuedTask::LanTimeSync { .. })) =>
                {
                    self.task_queue.push_back(QueuedTask::LanTimeSync {
                        recorded: false,
                        recorded_time: DnpTime::invalid(),
                    });
                }
                _ => {}
            }
        }
        if iin.is_set(IinBit::EventBufferOverflow)
            && self.params.integrity_on_event_overflow_iin
            && !self.has_queued(|t| matches!(t, QueuedTask::IntegrityPoll(_)))
        {
            self.task_queue
                .push_back(QueuedTask::IntegrityPoll(ClassField::ALL_CLASSES));
        }
        let mut avail = ClassField::NONE;
        if iin.is_set(IinBit::Class1Events) {
            avail.set(ClassField::CLASS_1);
        }
        if iin.is_set(IinBit::Class2Events) {
            avail.set(ClassField::CLASS_2);
        }
        if iin.is_set(IinBit::Class3Events) {
            avail.set(ClassField::CLASS_3);
        }
        let scan_mask = avail.intersect(self.params.event_scan_on_events_available_class_mask);
        if scan_mask.has_event_class()
            && !self.has_queued(|t| matches!(t, QueuedTask::EventScan(_)))
        {
            self.task_queue.push_back(QueuedTask::EventScan(scan_mask));
        }
    }

    fn has_queued(&self, pred: impl Fn(&QueuedTask) -> bool) -> bool {
        self.task_queue.iter().any(pred)
    }

    fn try_start_next_task(&mut self, now: Instant, app: &mut dyn MasterApplication) {
        if !self.is_online || self.active_task.is_some() || self.is_transmitting {
            return;
        }
        let Some(mut task) = self.task_queue.pop_front() else {
            return;
        };

        self.sol_seq.increment();
        let seq = self.sol_seq.value();
        let tt = task.task_type();
        app.on_task_start(tt);

        let bytes = match &mut task {
            QueuedTask::ClearRestart => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::Write,
                );
                w.write_iin_bit(IinBit::DeviceRestart as u8, false);
                w.into_bytes()
            }
            QueuedTask::DisableUnsol(classes) => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::DisableUnsolicited,
                );
                write_class_headers(&mut w, *classes);
                w.into_bytes()
            }
            QueuedTask::EnableUnsol(classes) => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::EnableUnsolicited,
                );
                write_class_headers(&mut w, *classes);
                w.into_bytes()
            }
            QueuedTask::IntegrityPoll(classes) | QueuedTask::EventScan(classes) => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::Read,
                );
                write_class_headers(&mut w, *classes);
                w.into_bytes()
            }
            QueuedTask::RangeScan(gv, range) => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::Read,
                );
                w.write_range_header(*gv, *range);
                w.into_bytes()
            }
            QueuedTask::AllObjectsScan(gv) => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::Read,
                );
                w.write_all_objects(*gv);
                w.into_bytes()
            }
            QueuedTask::AssignClass { class, gv, range } => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::AssignClass,
                );
                let class_gv = match class {
                    PointClass::Class0 => GroupVariation::Group60Var1,
                    PointClass::Class1 => GroupVariation::Group60Var2,
                    PointClass::Class2 => GroupVariation::Group60Var3,
                    PointClass::Class3 => GroupVariation::Group60Var4,
                };
                w.write_all_objects(class_gv);
                match range {
                    Some(r) => {
                        w.write_range_header(*gv, *r);
                    }
                    None => {
                        w.write_all_objects(*gv);
                    }
                }
                w.into_bytes()
            }
            QueuedTask::NonLanTimeSync { start_time, .. } => {
                *start_time = Some(now);
                ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::DelayMeasure,
                )
                .into_bytes()
            }
            QueuedTask::LanTimeSync { recorded_time, .. } => {
                *recorded_time = app.now_dnp_time();
                ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::RecordCurrentTime,
                )
                .into_bytes()
            }
            QueuedTask::SelectAndOperate {
                commands,
                select_payload,
                ..
            } => {
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    FunctionCode::Select,
                );
                commands.write(&mut w);
                let out = w.into_bytes();
                *select_payload = out[ApduHeader::SIZE..].to_vec();
                out
            }
            QueuedTask::DirectOperate { commands, no_ack } => {
                let fc = if *no_ack {
                    FunctionCode::DirectOperateNr
                } else {
                    FunctionCode::DirectOperate
                };
                let mut w = ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    fc,
                );
                commands.write(&mut w);
                w.into_bytes()
            }
            QueuedTask::Restart(rt) => {
                let fc = match rt {
                    RestartType::Cold => FunctionCode::ColdRestart,
                    RestartType::Warm => FunctionCode::WarmRestart,
                };
                ApduWriter::new_request(
                    self.params.max_tx_frag_size,
                    AppControlField::request(seq),
                    fc,
                )
                .into_bytes()
            }
        };

        let expects_response = !matches!(task, QueuedTask::DirectOperate { no_ack: true, .. });
        if expects_response {
            self.response_deadline = Some(now + self.params.response_timeout);
        }
        self.active_task = Some(task);
        self.tx_queue.push_back(Message::new(self.addresses, bytes));
    }

    fn finish_task_failure(
        &mut self,
        task: QueuedTask,
        reason: TaskCompletion,
        app: &mut dyn MasterApplication,
    ) {
        let tt = task.task_type();
        match task {
            QueuedTask::SelectAndOperate { .. } | QueuedTask::DirectOperate { .. } => {
                self.events
                    .push(MasterEvent::CommandComplete(CommandResult {
                        summary: reason,
                        points: Vec::new(),
                    }));
            }
            QueuedTask::Restart(_) => {
                self.events.push(MasterEvent::RestartComplete {
                    result: reason,
                    delay_ms: None,
                });
            }
            _ => {
                self.events.push(MasterEvent::TaskComplete {
                    task_type: tt,
                    result: reason,
                });
            }
        }
        app.on_task_complete(tt, reason);
    }

    fn dispatch_measurements(&self, objects: &[u8], info: ResponseInfo, soe: &mut dyn SoeHandler) {
        soe.begin_fragment(info);
        let mut adapter = SoeAdapter { soe };
        let _ = parse_objects(objects, ParseMode::WithObjects, &mut adapter);
        soe.end_fragment(info);
    }
}

fn write_class_headers(w: &mut ApduWriter, classes: ClassField) {
    if classes.has_class1() {
        w.write_all_objects(GroupVariation::Group60Var2);
    }
    if classes.has_class2() {
        w.write_all_objects(GroupVariation::Group60Var3);
    }
    if classes.has_class3() {
        w.write_all_objects(GroupVariation::Group60Var4);
    }
    if classes.has_class0() {
        w.write_all_objects(GroupVariation::Group60Var1);
    }
}

struct SoeAdapter<'a> {
    soe: &'a mut dyn SoeHandler,
}

impl ApduHandler for SoeAdapter<'_> {
    fn on_binaries(&mut self, info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        self.soe.process_binary(info, values);
        IinField::empty()
    }
    fn on_double_bits(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<DoubleBitBinary>],
    ) -> IinField {
        self.soe.process_double_bit(info, values);
        IinField::empty()
    }
    fn on_binary_output_statuses(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<BinaryOutputStatus>],
    ) -> IinField {
        self.soe.process_binary_output_status(info, values);
        IinField::empty()
    }
    fn on_counters(&mut self, info: HeaderInfo, values: &[Indexed<Counter>]) -> IinField {
        self.soe.process_counter(info, values);
        IinField::empty()
    }
    fn on_frozen_counters(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<FrozenCounter>],
    ) -> IinField {
        self.soe.process_frozen_counter(info, values);
        IinField::empty()
    }
    fn on_analogs(&mut self, info: HeaderInfo, values: &[Indexed<Analog>]) -> IinField {
        self.soe.process_analog(info, values);
        IinField::empty()
    }
    fn on_analog_output_statuses(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputStatus>],
    ) -> IinField {
        self.soe.process_analog_output_status(info, values);
        IinField::empty()
    }
    fn on_octet_strings(&mut self, info: HeaderInfo, values: &[Indexed<OctetString>]) -> IinField {
        self.soe.process_octet_string(info, values);
        IinField::empty()
    }
    fn on_time_and_intervals(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<TimeAndInterval>],
    ) -> IinField {
        self.soe.process_time_and_interval(info, values);
        IinField::empty()
    }
}

#[derive(Default)]
struct CommandResponseCollector {
    points: Vec<CommandPointResult>,
}

impl ApduHandler for CommandResponseCollector {
    fn on_crobs(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<ControlRelayOutputBlock>],
    ) -> IinField {
        for item in values {
            self.points.push(CommandPointResult {
                header_index: info.header_index,
                index: item.index,
                state: CommandPointState::Success,
                status: item.value.status,
            });
        }
        IinField::empty()
    }
    fn on_ao_i32s(&mut self, info: HeaderInfo, values: &[Indexed<AnalogOutputInt32>]) -> IinField {
        for item in values {
            self.points.push(CommandPointResult {
                header_index: info.header_index,
                index: item.index,
                state: CommandPointState::Success,
                status: item.value.status,
            });
        }
        IinField::empty()
    }
    fn on_ao_i16s(&mut self, info: HeaderInfo, values: &[Indexed<AnalogOutputInt16>]) -> IinField {
        for item in values {
            self.points.push(CommandPointResult {
                header_index: info.header_index,
                index: item.index,
                state: CommandPointState::Success,
                status: item.value.status,
            });
        }
        IinField::empty()
    }
    fn on_ao_f32s(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputFloat32>],
    ) -> IinField {
        for item in values {
            self.points.push(CommandPointResult {
                header_index: info.header_index,
                index: item.index,
                state: CommandPointState::Success,
                status: item.value.status,
            });
        }
        IinField::empty()
    }
    fn on_ao_f64s(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputDouble64>],
    ) -> IinField {
        for item in values {
            self.points.push(CommandPointResult {
                header_index: info.header_index,
                index: item.index,
                state: CommandPointState::Success,
                status: item.value.status,
            });
        }
        IinField::empty()
    }
}

#[derive(Default)]
struct DelayCollector {
    delay_ms: Option<u64>,
}

impl ApduHandler for DelayCollector {
    fn on_time_delay(&mut self, _info: HeaderInfo, fine: bool, values: &[u16]) -> IinField {
        if let Some(&v) = values.first() {
            self.delay_ms = Some(if fine { v as u64 } else { (v as u64) * 1000 });
        }
        IinField::empty()
    }
}
