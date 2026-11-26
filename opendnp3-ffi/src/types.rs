//! FFI-exported `#[data]` records and enums for Python, C#, Java, etc.

use boltffi::{data, error};
use std::time::Duration;

#[error]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    Closed,
    Io { message: String },
}

impl From<boltffi::UnexpectedFfiCallbackError> for StreamError {
    fn from(err: boltffi::UnexpectedFfiCallbackError) -> Self {
        Self::Io {
            message: format!("{err:?}"),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampQuality {
    Invalid,
    Synchronized,
    Unsynchronized,
}

impl From<opendnp3::enums::TimestampQuality> for TimestampQuality {
    fn from(q: opendnp3::enums::TimestampQuality) -> Self {
        match q {
            opendnp3::enums::TimestampQuality::Invalid => Self::Invalid,
            opendnp3::enums::TimestampQuality::Synchronized => Self::Synchronized,
            opendnp3::enums::TimestampQuality::Unsynchronized => Self::Unsynchronized,
        }
    }
}

impl From<TimestampQuality> for opendnp3::enums::TimestampQuality {
    fn from(q: TimestampQuality) -> Self {
        match q {
            TimestampQuality::Invalid => Self::Invalid,
            TimestampQuality::Synchronized => Self::Synchronized,
            TimestampQuality::Unsynchronized => Self::Unsynchronized,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnpTime {
    pub value_ms: u64,
    pub quality: TimestampQuality,
}

impl From<opendnp3::app::DnpTime> for DnpTime {
    fn from(t: opendnp3::app::DnpTime) -> Self {
        Self {
            value_ms: t.value,
            quality: t.quality.into(),
        }
    }
}

impl From<DnpTime> for opendnp3::app::DnpTime {
    fn from(t: DnpTime) -> Self {
        Self::with_quality(t.value_ms, t.quality.into())
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleBit {
    Intermediate,
    DeterminedOff,
    DeterminedOn,
    Indeterminate,
}

impl From<opendnp3::enums::DoubleBit> for DoubleBit {
    fn from(v: opendnp3::enums::DoubleBit) -> Self {
        match v {
            opendnp3::enums::DoubleBit::Intermediate => Self::Intermediate,
            opendnp3::enums::DoubleBit::DeterminedOff => Self::DeterminedOff,
            opendnp3::enums::DoubleBit::DeterminedOn => Self::DeterminedOn,
            opendnp3::enums::DoubleBit::Indeterminate => Self::Indeterminate,
        }
    }
}

impl From<DoubleBit> for opendnp3::enums::DoubleBit {
    fn from(v: DoubleBit) -> Self {
        match v {
            DoubleBit::Intermediate => Self::Intermediate,
            DoubleBit::DeterminedOff => Self::DeterminedOff,
            DoubleBit::DeterminedOn => Self::DeterminedOn,
            DoubleBit::Indeterminate => Self::Indeterminate,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventMode {
    Detect,
    Force,
    Suppress,
    EventOnly,
}

impl From<EventMode> for opendnp3::enums::EventMode {
    fn from(m: EventMode) -> Self {
        match m {
            EventMode::Detect => Self::Detect,
            EventMode::Force => Self::Force,
            EventMode::Suppress => Self::Suppress,
            EventMode::EventOnly => Self::EventOnly,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventClass {
    Class1,
    Class2,
    Class3,
}

impl From<EventClass> for opendnp3::enums::EventClass {
    fn from(c: EventClass) -> Self {
        match c {
            EventClass::Class1 => Self::Ec1,
            EventClass::Class2 => Self::Ec2,
            EventClass::Class3 => Self::Ec3,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointClass {
    Class0,
    Class1,
    Class2,
    Class3,
}

impl From<PointClass> for opendnp3::enums::PointClass {
    fn from(c: PointClass) -> Self {
        match c {
            PointClass::Class0 => Self::Class0,
            PointClass::Class1 => Self::Class1,
            PointClass::Class2 => Self::Class2,
            PointClass::Class3 => Self::Class3,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassField {
    pub class0: bool,
    pub class1: bool,
    pub class2: bool,
    pub class3: bool,
}

impl From<ClassField> for opendnp3::app::ClassField {
    fn from(c: ClassField) -> Self {
        let mut cf = opendnp3::app::ClassField::NONE;
        if c.class0 {
            cf.set(opendnp3::app::ClassField::CLASS_0);
        }
        if c.class1 {
            cf.set(opendnp3::app::ClassField::CLASS_1);
        }
        if c.class2 {
            cf.set(opendnp3::app::ClassField::CLASS_2);
        }
        if c.class3 {
            cf.set(opendnp3::app::ClassField::CLASS_3);
        }
        cf
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Nul,
    PulseOn,
    PulseOff,
    LatchOn,
    LatchOff,
    Undefined,
}

impl From<opendnp3::enums::OperationType> for OperationType {
    fn from(op: opendnp3::enums::OperationType) -> Self {
        match op {
            opendnp3::enums::OperationType::Nul => Self::Nul,
            opendnp3::enums::OperationType::PulseOn => Self::PulseOn,
            opendnp3::enums::OperationType::PulseOff => Self::PulseOff,
            opendnp3::enums::OperationType::LatchOn => Self::LatchOn,
            opendnp3::enums::OperationType::LatchOff => Self::LatchOff,
            opendnp3::enums::OperationType::Undefined => Self::Undefined,
        }
    }
}

impl From<OperationType> for opendnp3::enums::OperationType {
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

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripCloseCode {
    Nul,
    Close,
    Trip,
    Reserved,
}

impl From<opendnp3::enums::TripCloseCode> for TripCloseCode {
    fn from(c: opendnp3::enums::TripCloseCode) -> Self {
        match c {
            opendnp3::enums::TripCloseCode::Nul => Self::Nul,
            opendnp3::enums::TripCloseCode::Close => Self::Close,
            opendnp3::enums::TripCloseCode::Trip => Self::Trip,
            opendnp3::enums::TripCloseCode::Reserved => Self::Reserved,
        }
    }
}

impl From<TripCloseCode> for opendnp3::enums::TripCloseCode {
    fn from(c: TripCloseCode) -> Self {
        match c {
            TripCloseCode::Nul => Self::Nul,
            TripCloseCode::Close => Self::Close,
            TripCloseCode::Trip => Self::Trip,
            TripCloseCode::Reserved => Self::Reserved,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperateType {
    SelectBeforeOperate,
    DirectOperate,
    DirectOperateNoAck,
}

impl From<opendnp3::enums::OperateType> for OperateType {
    fn from(op: opendnp3::enums::OperateType) -> Self {
        match op {
            opendnp3::enums::OperateType::SelectBeforeOperate => Self::SelectBeforeOperate,
            opendnp3::enums::OperateType::DirectOperate => Self::DirectOperate,
            opendnp3::enums::OperateType::DirectOperateNoAck => Self::DirectOperateNoAck,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
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

impl From<opendnp3::enums::CommandStatus> for CommandStatus {
    fn from(s: opendnp3::enums::CommandStatus) -> Self {
        match s {
            opendnp3::enums::CommandStatus::Success => Self::Success,
            opendnp3::enums::CommandStatus::Timeout => Self::Timeout,
            opendnp3::enums::CommandStatus::NoSelect => Self::NoSelect,
            opendnp3::enums::CommandStatus::FormatError => Self::FormatError,
            opendnp3::enums::CommandStatus::NotSupported => Self::NotSupported,
            opendnp3::enums::CommandStatus::AlreadyActive => Self::AlreadyActive,
            opendnp3::enums::CommandStatus::HardwareError => Self::HardwareError,
            opendnp3::enums::CommandStatus::Local => Self::Local,
            opendnp3::enums::CommandStatus::TooManyOps => Self::TooManyOps,
            opendnp3::enums::CommandStatus::NotAuthorized => Self::NotAuthorized,
            opendnp3::enums::CommandStatus::AutomationInhibit => Self::AutomationInhibit,
            opendnp3::enums::CommandStatus::ProcessingLimited => Self::ProcessingLimited,
            opendnp3::enums::CommandStatus::OutOfRange => Self::OutOfRange,
            opendnp3::enums::CommandStatus::DownstreamLocal => Self::DownstreamLocal,
            opendnp3::enums::CommandStatus::AlreadyComplete => Self::AlreadyComplete,
            opendnp3::enums::CommandStatus::Blocked => Self::Blocked,
            opendnp3::enums::CommandStatus::Cancelled => Self::Cancelled,
            opendnp3::enums::CommandStatus::BlockedOtherMaster => Self::BlockedOtherMaster,
            opendnp3::enums::CommandStatus::DownstreamFail => Self::DownstreamFail,
            opendnp3::enums::CommandStatus::NonParticipating => Self::NonParticipating,
            opendnp3::enums::CommandStatus::Undefined => Self::Undefined,
        }
    }
}

impl From<CommandStatus> for opendnp3::enums::CommandStatus {
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

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartType {
    Cold,
    Warm,
}

impl From<RestartType> for opendnp3::enums::RestartType {
    fn from(r: RestartType) -> Self {
        match r {
            RestartType::Cold => Self::Cold,
            RestartType::Warm => Self::Warm,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSyncMode {
    None,
    NonLan,
    Lan,
}

impl From<TimeSyncMode> for opendnp3::enums::TimeSyncMode {
    fn from(m: TimeSyncMode) -> Self {
        match m {
            TimeSyncMode::None => Self::None,
            TimeSyncMode::NonLan => Self::NonLan,
            TimeSyncMode::Lan => Self::Lan,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskCompletion {
    Success,
    FailureBadResponse,
    FailureResponseTimeout,
    FailureStartTimeout,
    FailureMessageFormatError,
    FailureNoComms,
}

impl From<opendnp3::enums::TaskCompletion> for TaskCompletion {
    fn from(c: opendnp3::enums::TaskCompletion) -> Self {
        match c {
            opendnp3::enums::TaskCompletion::Success => Self::Success,
            opendnp3::enums::TaskCompletion::FailureBadResponse => Self::FailureBadResponse,
            opendnp3::enums::TaskCompletion::FailureResponseTimeout => Self::FailureResponseTimeout,
            opendnp3::enums::TaskCompletion::FailureStartTimeout => Self::FailureStartTimeout,
            opendnp3::enums::TaskCompletion::FailureMessageFormatError => {
                Self::FailureMessageFormatError
            }
            opendnp3::enums::TaskCompletion::FailureNoComms => Self::FailureNoComms,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterTaskType {
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

impl From<opendnp3::enums::MasterTaskType> for MasterTaskType {
    fn from(t: opendnp3::enums::MasterTaskType) -> Self {
        match t {
            opendnp3::enums::MasterTaskType::ClearRestart => Self::ClearRestart,
            opendnp3::enums::MasterTaskType::DisableUnsol => Self::DisableUnsol,
            opendnp3::enums::MasterTaskType::AssignClass => Self::AssignClass,
            opendnp3::enums::MasterTaskType::StartupIntegrityPoll => Self::StartupIntegrityPoll,
            opendnp3::enums::MasterTaskType::NonLanTimeSync => Self::NonLanTimeSync,
            opendnp3::enums::MasterTaskType::LanTimeSync => Self::LanTimeSync,
            opendnp3::enums::MasterTaskType::EnableUnsol => Self::EnableUnsol,
            opendnp3::enums::MasterTaskType::AutoEventScan => Self::AutoEventScan,
            opendnp3::enums::MasterTaskType::UserTask => Self::UserTask,
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPointState {
    Init,
    SelectFail,
    SelectTimeout,
    SelectMismatch,
    Success,
}

impl From<opendnp3::enums::CommandPointState> for CommandPointState {
    fn from(s: opendnp3::enums::CommandPointState) -> Self {
        match s {
            opendnp3::enums::CommandPointState::Init => Self::Init,
            opendnp3::enums::CommandPointState::SelectFail => Self::SelectFail,
            opendnp3::enums::CommandPointState::SelectTimeout => Self::SelectTimeout,
            opendnp3::enums::CommandPointState::SelectMismatch => Self::SelectMismatch,
            opendnp3::enums::CommandPointState::Success => Self::Success,
        }
    }
}

// ---- Measurements ----

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binary {
    pub index: u16,
    pub value: bool,
    pub flags: u8,
    pub time: DnpTime,
}

impl Binary {
    pub fn from_core(index: u16, v: opendnp3::app::Binary) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> opendnp3::app::Binary {
        opendnp3::app::Binary {
            value: self.value,
            flags: opendnp3::app::Flags::new(self.flags),
            time: self.time.into(),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoubleBitBinary {
    pub index: u16,
    pub value: DoubleBit,
    pub flags: u8,
    pub time: DnpTime,
}

impl DoubleBitBinary {
    pub fn from_core(index: u16, v: opendnp3::app::DoubleBitBinary) -> Self {
        Self {
            index,
            value: v.value.into(),
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> opendnp3::app::DoubleBitBinary {
        opendnp3::app::DoubleBitBinary {
            value: self.value.into(),
            flags: opendnp3::app::Flags::new(self.flags),
            time: self.time.into(),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryOutputStatus {
    pub index: u16,
    pub value: bool,
    pub flags: u8,
    pub time: DnpTime,
}

impl BinaryOutputStatus {
    pub fn from_core(index: u16, v: opendnp3::app::BinaryOutputStatus) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> opendnp3::app::BinaryOutputStatus {
        opendnp3::app::BinaryOutputStatus {
            value: self.value,
            flags: opendnp3::app::Flags::new(self.flags),
            time: self.time.into(),
        }
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counter {
    pub index: u16,
    pub value: u32,
    pub flags: u8,
    pub time: DnpTime,
}

impl Counter {
    pub fn from_core(index: u16, v: opendnp3::app::Counter) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> opendnp3::app::Counter {
        opendnp3::app::Counter::with_flags_and_time(
            self.value,
            opendnp3::app::Flags::new(self.flags),
            self.time.into(),
        )
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenCounter {
    pub index: u16,
    pub value: u32,
    pub flags: u8,
    pub time: DnpTime,
}

impl FrozenCounter {
    pub fn from_core(index: u16, v: opendnp3::app::FrozenCounter) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> opendnp3::app::FrozenCounter {
        opendnp3::app::FrozenCounter::with_flags_and_time(
            self.value,
            opendnp3::app::Flags::new(self.flags),
            self.time.into(),
        )
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Analog {
    pub index: u16,
    pub value: f64,
    pub flags: u8,
    pub time: DnpTime,
}

impl Analog {
    pub fn from_core(index: u16, v: opendnp3::app::Analog) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> opendnp3::app::Analog {
        opendnp3::app::Analog::with_flags_and_time(
            self.value,
            opendnp3::app::Flags::new(self.flags),
            self.time.into(),
        )
    }
}

#[data]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogOutputStatus {
    pub index: u16,
    pub value: f64,
    pub flags: u8,
    pub time: DnpTime,
}

impl AnalogOutputStatus {
    pub fn from_core(index: u16, v: opendnp3::app::AnalogOutputStatus) -> Self {
        Self {
            index,
            value: v.value,
            flags: v.flags.value,
            time: v.time.into(),
        }
    }
    pub fn into_core(self) -> opendnp3::app::AnalogOutputStatus {
        opendnp3::app::AnalogOutputStatus::with_flags_and_time(
            self.value,
            opendnp3::app::Flags::new(self.flags),
            self.time.into(),
        )
    }
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OctetString {
    pub index: u16,
    pub bytes: Vec<u8>,
}

impl OctetString {
    pub fn from_core(index: u16, v: opendnp3::app::OctetString) -> Self {
        Self {
            index,
            bytes: v.bytes,
        }
    }
    pub fn into_core(self) -> opendnp3::app::OctetString {
        opendnp3::app::OctetString::new(self.bytes)
    }
}

// ---- Commands ----

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crob {
    pub op_type: OperationType,
    pub tcc: TripCloseCode,
    pub clear: bool,
    pub count: u8,
    pub on_time_ms: u32,
    pub off_time_ms: u32,
    pub status: CommandStatus,
}

impl From<opendnp3::app::ControlRelayOutputBlock> for Crob {
    fn from(c: opendnp3::app::ControlRelayOutputBlock) -> Self {
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

impl From<Crob> for opendnp3::app::ControlRelayOutputBlock {
    fn from(c: Crob) -> Self {
        opendnp3::app::ControlRelayOutputBlock::new(
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
pub struct CommandPointResult {
    pub header_index: u32,
    pub index: u16,
    pub state: CommandPointState,
    pub status: CommandStatus,
}

impl From<opendnp3::master::CommandPointResult> for CommandPointResult {
    fn from(r: opendnp3::master::CommandPointResult) -> Self {
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
pub struct CommandResult {
    pub summary: TaskCompletion,
    pub points: Vec<CommandPointResult>,
}

impl From<opendnp3::master::CommandResult> for CommandResult {
    fn from(r: opendnp3::master::CommandResult) -> Self {
        Self {
            summary: r.summary.into(),
            points: r.points.into_iter().map(Into::into).collect(),
        }
    }
}

#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterEvent {
    TaskComplete {
        task_type: MasterTaskType,
        result: TaskCompletion,
    },
    CommandComplete {
        result: CommandResult,
    },
    RestartComplete {
        result: TaskCompletion,
        delay_ms: Option<u64>,
    },
}

impl From<opendnp3::master::MasterEvent> for MasterEvent {
    fn from(ev: opendnp3::master::MasterEvent) -> Self {
        match ev {
            opendnp3::master::MasterEvent::TaskComplete { task_type, result } => {
                Self::TaskComplete {
                    task_type: task_type.into(),
                    result: result.into(),
                }
            }
            opendnp3::master::MasterEvent::CommandComplete(r) => {
                Self::CommandComplete { result: r.into() }
            }
            opendnp3::master::MasterEvent::RestartComplete { result, delay_ms } => {
                Self::RestartComplete {
                    result: result.into(),
                    delay_ms,
                }
            }
        }
    }
}

// ---- Configs ----

#[data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkConfig {
    pub is_master: bool,
    pub local_addr: u16,
    pub remote_addr: u16,
    pub timeout_ms: u64,
    pub keep_alive_timeout_ms: u64,
    pub respond_to_any_source: bool,
}

impl From<LinkConfig> for opendnp3::link::LinkConfig {
    fn from(c: LinkConfig) -> Self {
        opendnp3::link::LinkConfig {
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
pub struct MasterParams {
    pub response_timeout_ms: u64,
    pub time_sync_mode: TimeSyncMode,
    pub disable_unsol_on_startup: bool,
    pub ignore_restart_iin: bool,
    pub unsol_class_mask: ClassField,
    pub startup_integrity_class_mask: ClassField,
    pub integrity_on_event_overflow_iin: bool,
    pub event_scan_on_events_available_class_mask: ClassField,
    pub max_tx_frag_size: u32,
    pub max_rx_frag_size: u32,
}

impl From<MasterParams> for opendnp3::master::MasterParams {
    fn from(p: MasterParams) -> Self {
        opendnp3::master::MasterParams {
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
pub struct OutstationParams {
    pub max_tx_frag_size: u32,
    pub max_rx_frag_size: u32,
    pub allow_unsolicited: bool,
    pub sol_confirm_timeout_ms: u64,
    pub unsol_confirm_timeout_ms: u64,
    pub unsol_retry_delay_ms: u64,
    pub select_timeout_ms: u64,
    pub max_controls_per_request: u32,
    pub unsol_class_mask: ClassField,
    pub respond_to_any_master: bool,
    pub event_buffer_size_per_type: u32,
}

impl OutstationParams {
    pub fn into_core(
        self,
    ) -> (
        opendnp3::outstation::OutstationParams,
        opendnp3::outstation::EventBufferConfig,
    ) {
        let params = opendnp3::outstation::OutstationParams {
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
        let events = opendnp3::outstation::EventBufferConfig::all_types(
            self.event_buffer_size_per_type as usize,
        );
        (params, events)
    }
}

pub fn gv_from_raw(group: u8, variation: u8) -> opendnp3::enums::GroupVariation {
    opendnp3::enums::GroupVariation::from_group_var(group, variation)
}

pub fn range_from_raw(start: u16, stop: u16) -> opendnp3::app::Range {
    opendnp3::app::Range::new(start, stop)
}

pub fn ao_i32(value: i32) -> opendnp3::app::AnalogOutputInt32 {
    opendnp3::app::AnalogOutputInt32::new(value)
}

pub fn ao_i16(value: i16) -> opendnp3::app::AnalogOutputInt16 {
    opendnp3::app::AnalogOutputInt16::new(value)
}

pub fn ao_f32(value: f32) -> opendnp3::app::AnalogOutputFloat32 {
    opendnp3::app::AnalogOutputFloat32::new(value)
}

pub fn ao_f64(value: f64) -> opendnp3::app::AnalogOutputDouble64 {
    opendnp3::app::AnalogOutputDouble64::new(value)
}
