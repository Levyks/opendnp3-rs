//! Master task lifecycle and command result enumerations.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskCompletion {
    Success,
    FailureBadResponse,
    FailureResponseTimeout,
    FailureStartTimeout,
    FailureMessageFormatError,
    FailureNoComms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TimeSyncMode {
    #[default]
    None,
    NonLan,
    Lan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandPointState {
    Init,
    SelectFail,
    SelectTimeout,
    SelectMismatch,
    Success,
}
