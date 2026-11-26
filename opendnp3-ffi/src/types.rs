//! FFI-exported `#[data]` records and enums for Python, C#, Java, etc.

use boltffi::{data, error};
use opendnp3::app::{
    Analog, AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    AnalogOutputStatus, Binary, BinaryOutputStatus, ClassField, ControlRelayOutputBlock, Counter,
    DnpTime, DoubleBitBinary, Flags, FrozenCounter, OctetString, Range,
};
use opendnp3::enums::{
    CommandPointState, CommandStatus, DoubleBit, EventClass, EventMode, GroupVariation,
    MasterTaskType, OperateType, OperationType, PointClass, RestartType, TaskCompletion,
    TimeSyncMode, TimestampQuality, TripCloseCode,
};
use opendnp3::link::LinkConfig;
use opendnp3::master::{CommandPointResult, CommandResult, MasterEvent, MasterParams};
use opendnp3::outstation::{EventBufferConfig, OutstationParams};
use std::time::Duration;

#[error]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfiStreamError {
    Closed,
    Io { message: String },
}

impl From<boltffi::UnexpectedFfiCallbackError> for FfiStreamError {
    fn from(err: boltffi::UnexpectedFfiCallbackError) -> Self {
        Self::Io {
            message: format!("{err:?}"),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiTimestampQuality {
    Invalid,
    Synchronized,
    Unsynchronized,
}

impl From<TimestampQuality> for FfiTimestampQuality {
    fn from(q: TimestampQuality) -> Self {
        match q {
            TimestampQuality::Invalid => Self::Invalid,
            TimestampQuality::Synchronized => Self::Synchronized,
            TimestampQuality::Unsynchronized => Self::Unsynchronized,
        }
    }
}

impl From<FfiTimestampQuality> for TimestampQuality {
    fn from(q: FfiTimestampQuality) -> Self {
        match q {
            FfiTimestampQuality::Invalid => Self::Invalid,
            FfiTimestampQuality::Synchronized => Self::Synchronized,
            FfiTimestampQuality::Unsynchronized => Self::Unsynchronized,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiDnpTime {
    pub value_ms: u64,
    pub quality: FfiTimestampQuality,
}

impl From<DnpTime> for FfiDnpTime {
    fn from(t: DnpTime) -> Self {
        Self {
            value_ms: t.value,
            quality: t.quality.into(),
        }
    }
}

impl From<FfiDnpTime> for DnpTime {
    fn from(t: FfiDnpTime) -> Self {
        Self::with_quality(t.value_ms, t.quality.into())
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiDoubleBit {
    Intermediate,
    DeterminedOff,
    DeterminedOn,
    Indeterminate,
}

impl From<DoubleBit> for FfiDoubleBit {
    fn from(v: DoubleBit) -> Self {
        match v {
            DoubleBit::Intermediate => Self::Intermediate,
            DoubleBit::DeterminedOff => Self::DeterminedOff,
            DoubleBit::DeterminedOn => Self::DeterminedOn,
            DoubleBit::Indeterminate => Self::Indeterminate,
        }
    }
}

impl From<FfiDoubleBit> for DoubleBit {
    fn from(v: FfiDoubleBit) -> Self {
        match v {
            FfiDoubleBit::Intermediate => Self::Intermediate,
            FfiDoubleBit::DeterminedOff => Self::DeterminedOff,
            FfiDoubleBit::DeterminedOn => Self::DeterminedOn,
            FfiDoubleBit::Indeterminate => Self::Indeterminate,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiEventMode {
    Detect,
    Force,
    Suppress,
    EventOnly,
}

impl From<FfiEventMode> for EventMode {
    fn from(m: FfiEventMode) -> Self {
        match m {
            FfiEventMode::Detect => Self::Detect,
            FfiEventMode::Force => Self::Force,
            FfiEventMode::Suppress => Self::Suppress,
            FfiEventMode::EventOnly => Self::EventOnly,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiEventClass {
    Class1,
    Class2,
    Class3,
}

impl From<FfiEventClass> for EventClass {
    fn from(c: FfiEventClass) -> Self {
        match c {
            FfiEventClass::Class1 => Self::Ec1,
            FfiEventClass::Class2 => Self::Ec2,
            FfiEventClass::Class3 => Self::Ec3,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiPointClass {
    Class0,
    Class1,
    Class2,
    Class3,
}

impl From<FfiPointClass> for PointClass {
    fn from(c: FfiPointClass) -> Self {
        match c {
            FfiPointClass::Class0 => Self::Class0,
            FfiPointClass::Class1 => Self::Class1,
            FfiPointClass::Class2 => Self::Class2,
            FfiPointClass::Class3 => Self::Class3,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiClassField {
    pub class0: bool,
    pub class1: bool,
    pub class2: bool,
    pub class3: bool,
}

impl From<FfiClassField> for ClassField {
    fn from(c: FfiClassField) -> Self {
        let mut cf = ClassField::NONE;
        if c.class0 {
            cf.set(ClassField::CLASS_0);
        }
        if c.class1 {
            cf.set(ClassField::CLASS_1);
        }
        if c.class2 {
            cf.set(ClassField::CLASS_2);
        }
        if c.class3 {
            cf.set(ClassField::CLASS_3);
        }
        cf
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiOperationType {
    Nul,
    PulseOn,
    PulseOff,
    LatchOn,
    LatchOff,
    Undefined,
}

impl From<OperationType> for FfiOperationType {
    fn from(op: OperationType) -> Self {
        match op {
            OperationType::Nul => Self::Nul,
            OperationType::PulseOn => Self::PulseOn,
            OperationType::PulseOff => Self::PulseOff,
            OperationType::LatchOn => Self::LatchOn,
            OperationType::LatchOff => Self::LatchOff,
            OperationType::Undefined => Self::Undefined,
        }
    }
}

impl From<FfiOperationType> for OperationType {
    fn from(op: FfiOperationType) -> Self {
        match op {
            FfiOperationType::Nul => Self::Nul,
            FfiOperationType::PulseOn => Self::PulseOn,
            FfiOperationType::PulseOff => Self::PulseOff,
            FfiOperationType::LatchOn => Self::LatchOn,
            FfiOperationType::LatchOff => Self::LatchOff,
            FfiOperationType::Undefined => Self::Undefined,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiTripCloseCode {
    Nul,
    Close,
    Trip,
    Reserved,
}

impl From<TripCloseCode> for FfiTripCloseCode {
    fn from(c: TripCloseCode) -> Self {
        match c {
            TripCloseCode::Nul => Self::Nul,
            TripCloseCode::Close => Self::Close,
            TripCloseCode::Trip => Self::Trip,
            TripCloseCode::Reserved => Self::Reserved,
        }
    }
}

impl From<FfiTripCloseCode> for TripCloseCode {
    fn from(c: FfiTripCloseCode) -> Self {
        match c {
            FfiTripCloseCode::Nul => Self::Nul,
            FfiTripCloseCode::Close => Self::Close,
            FfiTripCloseCode::Trip => Self::Trip,
            FfiTripCloseCode::Reserved => Self::Reserved,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiOperateType {
    SelectBeforeOperate,
    DirectOperate,
    DirectOperateNoAck,
}

impl From<OperateType> for FfiOperateType {
    fn from(op: OperateType) -> Self {
        match op {
            OperateType::SelectBeforeOperate => Self::SelectBeforeOperate,
            OperateType::DirectOperate => Self::DirectOperate,
            OperateType::DirectOperateNoAck => Self::DirectOperateNoAck,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiCommandStatus {
    Success,
    Timeout,
    NoSelect,
    FormatError,
    NotSupported,
    AlreadyActive,
    HardwareError,
    Local,
    TooManyOps,
    NotAuthorized,
    AutomationInhibit,
    ProcessingLimited,
    OutOfRange,
    DownstreamLocal,
    AlreadyComplete,
    Blocked,
    Cancelled,
    BlockedOtherMaster,
    DownstreamFail,
    NonParticipating,
    Undefined,
}

impl From<CommandStatus> for FfiCommandStatus {
    fn from(s: CommandStatus) -> Self {
        match s {
            CommandStatus::Success => Self::Success,
            CommandStatus::Timeout => Self::Timeout,
            CommandStatus::NoSelect => Self::NoSelect,
            CommandStatus::FormatError => Self::FormatError,
            CommandStatus::NotSupported => Self::NotSupported,
            CommandStatus::AlreadyActive => Self::AlreadyActive,
            CommandStatus::HardwareError => Self::HardwareError,
            CommandStatus::Local => Self::Local,
            CommandStatus::TooManyOps => Self::TooManyOps,
            CommandStatus::NotAuthorized => Self::NotAuthorized,
            CommandStatus::AutomationInhibit => Self::AutomationInhibit,
            CommandStatus::ProcessingLimited => Self::ProcessingLimited,
            CommandStatus::OutOfRange => Self::OutOfRange,
            CommandStatus::DownstreamLocal => Self::DownstreamLocal,
            CommandStatus::AlreadyComplete => Self::AlreadyComplete,
            CommandStatus::Blocked => Self::Blocked,
            CommandStatus::Cancelled => Self::Cancelled,
            CommandStatus::BlockedOtherMaster => Self::BlockedOtherMaster,
            CommandStatus::DownstreamFail => Self::DownstreamFail,
            CommandStatus::NonParticipating => Self::NonParticipating,
            CommandStatus::Undefined => Self::Undefined,
        }
    }
}

impl From<FfiCommandStatus> for CommandStatus {
    fn from(s: FfiCommandStatus) -> Self {
        match s {
            FfiCommandStatus::Success => Self::Success,
            FfiCommandStatus::Timeout => Self::Timeout,
            FfiCommandStatus::NoSelect => Self::NoSelect,
            FfiCommandStatus::FormatError => Self::FormatError,
            FfiCommandStatus::NotSupported => Self::NotSupported,
            FfiCommandStatus::AlreadyActive => Self::AlreadyActive,
            FfiCommandStatus::HardwareError => Self::HardwareError,
            FfiCommandStatus::Local => Self::Local,
            FfiCommandStatus::TooManyOps => Self::TooManyOps,
            FfiCommandStatus::NotAuthorized => Self::NotAuthorized,
            FfiCommandStatus::AutomationInhibit => Self::AutomationInhibit,
            FfiCommandStatus::ProcessingLimited => Self::ProcessingLimited,
            FfiCommandStatus::OutOfRange => Self::OutOfRange,
            FfiCommandStatus::DownstreamLocal => Self::DownstreamLocal,
            FfiCommandStatus::AlreadyComplete => Self::AlreadyComplete,
            FfiCommandStatus::Blocked => Self::Blocked,
            FfiCommandStatus::Cancelled => Self::Cancelled,
            FfiCommandStatus::BlockedOtherMaster => Self::BlockedOtherMaster,
            FfiCommandStatus::DownstreamFail => Self::DownstreamFail,
            FfiCommandStatus::NonParticipating => Self::NonParticipating,
            FfiCommandStatus::Undefined => Self::Undefined,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiRestartType {
    Cold,
    Warm,
}

impl From<FfiRestartType> for RestartType {
    fn from(r: FfiRestartType) -> Self {
        match r {
            FfiRestartType::Cold => Self::Cold,
            FfiRestartType::Warm => Self::Warm,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiTimeSyncMode {
    None,
    NonLan,
    Lan,
}

impl From<FfiTimeSyncMode> for TimeSyncMode {
    fn from(m: FfiTimeSyncMode) -> Self {
        match m {
            FfiTimeSyncMode::None => Self::None,
            FfiTimeSyncMode::NonLan => Self::NonLan,
            FfiTimeSyncMode::Lan => Self::Lan,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiTaskCompletion {
    Success,
    FailureBadResponse,
    FailureResponseTimeout,
    FailureStartTimeout,
    FailureMessageFormatError,
    FailureNoComms,
}

impl From<TaskCompletion> for FfiTaskCompletion {
    fn from(c: TaskCompletion) -> Self {
        match c {
            TaskCompletion::Success => Self::Success,
            TaskCompletion::FailureBadResponse => Self::FailureBadResponse,
            TaskCompletion::FailureResponseTimeout => Self::FailureResponseTimeout,
            TaskCompletion::FailureStartTimeout => Self::FailureStartTimeout,
            TaskCompletion::FailureMessageFormatError => Self::FailureMessageFormatError,
            TaskCompletion::FailureNoComms => Self::FailureNoComms,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiMasterTaskType {
    ClearRestart,
    DisableUnsol,
    AssignClass,
    StartupIntegrityPoll,
    NonLanTimeSync,
    LanTimeSync,
    EnableUnsol,
    AutoEventScan,
    UserTask,
}

impl From<MasterTaskType> for FfiMasterTaskType {
    fn from(t: MasterTaskType) -> Self {
        match t {
            MasterTaskType::ClearRestart => Self::ClearRestart,
            MasterTaskType::DisableUnsol => Self::DisableUnsol,
            MasterTaskType::AssignClass => Self::AssignClass,
            MasterTaskType::StartupIntegrityPoll => Self::StartupIntegrityPoll,
            MasterTaskType::NonLanTimeSync => Self::NonLanTimeSync,
            MasterTaskType::LanTimeSync => Self::LanTimeSync,
            MasterTaskType::EnableUnsol => Self::EnableUnsol,
            MasterTaskType::AutoEventScan => Self::AutoEventScan,
            MasterTaskType::UserTask => Self::UserTask,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiCommandPointState {
    Init,
    SelectFail,
    SelectTimeout,
    SelectMismatch,
    Success,
}

impl From<CommandPointState> for FfiCommandPointState {
    fn from(s: CommandPointState) -> Self {
        match s {
            CommandPointState::Init => Self::Init,
            CommandPointState::SelectFail => Self::SelectFail,
            CommandPointState::SelectTimeout => Self::SelectTimeout,
            CommandPointState::SelectMismatch => Self::SelectMismatch,
            CommandPointState::Success => Self::Success,
        }
    }
}

// ---- Measurements ----

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiBinary {
    pub index: u16,
    pub value: bool,
    pub flags: u8,
    pub time: FfiDnpTime,
}

impl FfiBinary {
    pub fn from_core(index: u16, v: Binary) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> Binary {
        Binary {
            value: self.value,
            flags: Flags::new(self.flags),
            time: self.time.into(),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiDoubleBitBinary {
    pub index: u16,
    pub value: FfiDoubleBit,
    pub flags: u8,
    pub time: FfiDnpTime,
}

impl FfiDoubleBitBinary {
    pub fn from_core(index: u16, v: DoubleBitBinary) -> Self {
        Self {
            index,
            value: v.value.into(),
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> DoubleBitBinary {
        DoubleBitBinary {
            value: self.value.into(),
            flags: Flags::new(self.flags),
            time: self.time.into(),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiBinaryOutputStatus {
    pub index: u16,
    pub value: bool,
    pub flags: u8,
    pub time: FfiDnpTime,
}

impl FfiBinaryOutputStatus {
    pub fn from_core(index: u16, v: BinaryOutputStatus) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> BinaryOutputStatus {
        BinaryOutputStatus {
            value: self.value,
            flags: Flags::new(self.flags),
            time: self.time.into(),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiCounter {
    pub index: u16,
    pub value: u32,
    pub flags: u8,
    pub time: FfiDnpTime,
}

impl FfiCounter {
    pub fn from_core(index: u16, v: Counter) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> Counter {
        Counter::with_flags_and_time(self.value, Flags::new(self.flags), self.time.into())
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiFrozenCounter {
    pub index: u16,
    pub value: u32,
    pub flags: u8,
    pub time: FfiDnpTime,
}

impl FfiFrozenCounter {
    pub fn from_core(index: u16, v: FrozenCounter) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> FrozenCounter {
        FrozenCounter::with_flags_and_time(self.value, Flags::new(self.flags), self.time.into())
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FfiAnalog {
    pub index: u16,
    pub value: f64,
    pub flags: u8,
    pub time: FfiDnpTime,
}

impl FfiAnalog {
    pub fn from_core(index: u16, v: Analog) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> Analog {
        Analog::with_flags_and_time(self.value, Flags::new(self.flags), self.time.into())
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FfiAnalogOutputStatus {
    pub index: u16,
    pub value: f64,
    pub flags: u8,
    pub time: FfiDnpTime,
}

impl FfiAnalogOutputStatus {
    pub fn from_core(index: u16, v: AnalogOutputStatus) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> AnalogOutputStatus {
        AnalogOutputStatus::with_flags_and_time(
            self.value,
            Flags::new(self.flags),
            self.time.into(),
        )
    }
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiOctetString {
    pub index: u16,
    pub bytes: Vec<u8>,
}

impl FfiOctetString {
    pub fn from_core(index: u16, v: OctetString) -> Self {
        Self {
            index,
            bytes: v.bytes,
        }
    }
    pub fn into_core(self) -> OctetString {
        OctetString::new(self.bytes)
    }
}

// ---- Commands ----

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiCrob {
    pub op_type: FfiOperationType,
    pub tcc: FfiTripCloseCode,
    pub clear: bool,
    pub count: u8,
    pub on_time_ms: u32,
    pub off_time_ms: u32,
    pub status: FfiCommandStatus,
}

impl From<ControlRelayOutputBlock> for FfiCrob {
    fn from(c: ControlRelayOutputBlock) -> Self {
        Self {
            op_type: c.op_type.into(),
            tcc: c.tcc.into(),
            clear: c.clear,
            count: c.count,
            on_time_ms: c.on_time_ms,
            off_time_ms: c.off_time_ms,
            status: c.status.into(),
        }
    }
}

impl From<FfiCrob> for ControlRelayOutputBlock {
    fn from(c: FfiCrob) -> Self {
        ControlRelayOutputBlock::new(
            c.op_type.into(),
            c.tcc.into(),
            c.clear,
            c.count,
            c.on_time_ms,
            c.off_time_ms,
            c.status.into(),
        )
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiCommandPointResult {
    pub header_index: u32,
    pub index: u16,
    pub state: FfiCommandPointState,
    pub status: FfiCommandStatus,
}

impl From<CommandPointResult> for FfiCommandPointResult {
    fn from(r: CommandPointResult) -> Self {
        Self {
            header_index: r.header_index as u32,
            index: r.index,
            state: r.state.into(),
            status: r.status.into(),
        }
    }
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfiCommandResult {
    pub summary: FfiTaskCompletion,
    pub points: Vec<FfiCommandPointResult>,
}

impl From<CommandResult> for FfiCommandResult {
    fn from(r: CommandResult) -> Self {
        Self {
            summary: r.summary.into(),
            points: r.points.into_iter().map(Into::into).collect(),
        }
    }
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfiMasterEvent {
    TaskComplete {
        task_type: FfiMasterTaskType,
        result: FfiTaskCompletion,
    },
    CommandComplete {
        result: FfiCommandResult,
    },
    RestartComplete {
        result: FfiTaskCompletion,
        delay_ms: Option<u64>,
    },
}

impl From<MasterEvent> for FfiMasterEvent {
    fn from(ev: MasterEvent) -> Self {
        match ev {
            MasterEvent::TaskComplete { task_type, result } => Self::TaskComplete {
                task_type: task_type.into(),
                result: result.into(),
            },
            MasterEvent::CommandComplete(r) => Self::CommandComplete { result: r.into() },
            MasterEvent::RestartComplete { result, delay_ms } => Self::RestartComplete {
                result: result.into(),
                delay_ms,
            },
        }
    }
}

// ---- Configs ----

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiLinkConfig {
    pub is_master: bool,
    pub local_addr: u16,
    pub remote_addr: u16,
    pub timeout_ms: u64,
    pub keep_alive_timeout_ms: u64,
    pub respond_to_any_source: bool,
}

impl From<FfiLinkConfig> for LinkConfig {
    fn from(c: FfiLinkConfig) -> Self {
        LinkConfig {
            is_master: c.is_master,
            local_addr: c.local_addr,
            remote_addr: c.remote_addr,
            timeout: Duration::from_millis(c.timeout_ms),
            keep_alive_timeout: if c.keep_alive_timeout_ms == u64::MAX {
                Duration::MAX
            } else {
                Duration::from_millis(c.keep_alive_timeout_ms)
            },
            respond_to_any_source: c.respond_to_any_source,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiMasterParams {
    pub response_timeout_ms: u64,
    pub time_sync_mode: FfiTimeSyncMode,
    pub disable_unsol_on_startup: bool,
    pub ignore_restart_iin: bool,
    pub unsol_class_mask: FfiClassField,
    pub startup_integrity_class_mask: FfiClassField,
    pub integrity_on_event_overflow_iin: bool,
    pub event_scan_on_events_available_class_mask: FfiClassField,
    pub max_tx_frag_size: u32,
    pub max_rx_frag_size: u32,
}

impl From<FfiMasterParams> for MasterParams {
    fn from(p: FfiMasterParams) -> Self {
        MasterParams {
            response_timeout: Duration::from_millis(p.response_timeout_ms),
            time_sync_mode: p.time_sync_mode.into(),
            disable_unsol_on_startup: p.disable_unsol_on_startup,
            ignore_restart_iin: p.ignore_restart_iin,
            unsol_class_mask: p.unsol_class_mask.into(),
            startup_integrity_class_mask: p.startup_integrity_class_mask.into(),
            integrity_on_event_overflow_iin: p.integrity_on_event_overflow_iin,
            event_scan_on_events_available_class_mask: p
                .event_scan_on_events_available_class_mask
                .into(),
            max_tx_frag_size: p.max_tx_frag_size as usize,
            max_rx_frag_size: p.max_rx_frag_size as usize,
            ..Default::default()
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FfiOutstationParams {
    pub max_tx_frag_size: u32,
    pub max_rx_frag_size: u32,
    pub allow_unsolicited: bool,
    pub sol_confirm_timeout_ms: u64,
    pub unsol_confirm_timeout_ms: u64,
    pub unsol_retry_delay_ms: u64,
    pub select_timeout_ms: u64,
    pub max_controls_per_request: u32,
    pub unsol_class_mask: FfiClassField,
    pub respond_to_any_master: bool,
    pub event_buffer_size_per_type: u32,
}

impl FfiOutstationParams {
    pub fn into_core(self) -> (OutstationParams, EventBufferConfig) {
        let params = OutstationParams {
            max_tx_frag_size: self.max_tx_frag_size as usize,
            max_rx_frag_size: self.max_rx_frag_size as usize,
            allow_unsolicited: self.allow_unsolicited,
            sol_confirm_timeout: Duration::from_millis(self.sol_confirm_timeout_ms),
            unsol_confirm_timeout: Duration::from_millis(self.unsol_confirm_timeout_ms),
            unsol_retry_delay: Duration::from_millis(self.unsol_retry_delay_ms),
            select_timeout: Duration::from_millis(self.select_timeout_ms),
            max_controls_per_request: self.max_controls_per_request as usize,
            unsol_class_mask: self.unsol_class_mask.into(),
            respond_to_any_master: self.respond_to_any_master,
        };
        let events = EventBufferConfig::all_types(self.event_buffer_size_per_type as usize);
        (params, events)
    }
}

pub fn gv_from_raw(group: u8, variation: u8) -> GroupVariation {
    GroupVariation::from_group_var(group, variation)
}

pub fn range_from_raw(start: u16, stop: u16) -> Range {
    Range::new(start, stop)
}

pub fn ao_i32(value: i32) -> AnalogOutputInt32 {
    AnalogOutputInt32::new(value)
}

pub fn ao_i16(value: i16) -> AnalogOutputInt16 {
    AnalogOutputInt16::new(value)
}

pub fn ao_f32(value: f32) -> AnalogOutputFloat32 {
    AnalogOutputFloat32::new(value)
}

pub fn ao_f64(value: f64) -> AnalogOutputDouble64 {
    AnalogOutputDouble64::new(value)
}
