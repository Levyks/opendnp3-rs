//! Master callback traits (`SoeHandler`, `MasterApplication`).

use alloc::vec::Vec;

use crate::app::{
    Analog, AnalogOutputStatus, Binary, BinaryOutputStatus, Counter, DnpTime, DoubleBitBinary,
    FrozenCounter, HeaderInfo, IinField, Indexed, OctetString, TimeAndInterval,
};
use crate::enums::{CommandPointState, CommandStatus, MasterTaskType, TaskCompletion};

/// Metadata about the response fragment being delivered to [`SoeHandler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseInfo {
    pub unsolicited: bool,
    pub fir: bool,
    pub fin: bool,
}

/// Sequence-of-Events / measurement receiver implemented by master users.
#[allow(unused_variables)]
pub trait SoeHandler {
    fn begin_fragment(&mut self, info: ResponseInfo) {}
    fn end_fragment(&mut self, info: ResponseInfo) {}

    fn process_binary(&mut self, info: HeaderInfo, values: &[Indexed<Binary>]) {}
    fn process_double_bit(&mut self, info: HeaderInfo, values: &[Indexed<DoubleBitBinary>]) {}
    fn process_binary_output_status(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<BinaryOutputStatus>],
    ) {
    }
    fn process_counter(&mut self, info: HeaderInfo, values: &[Indexed<Counter>]) {}
    fn process_frozen_counter(&mut self, info: HeaderInfo, values: &[Indexed<FrozenCounter>]) {}
    fn process_analog(&mut self, info: HeaderInfo, values: &[Indexed<Analog>]) {}
    fn process_analog_output_status(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputStatus>],
    ) {
    }
    fn process_octet_string(&mut self, info: HeaderInfo, values: &[Indexed<OctetString>]) {}
    fn process_time_and_interval(&mut self, info: HeaderInfo, values: &[Indexed<TimeAndInterval>]) {
    }
}

/// No-op [`SoeHandler`].
#[derive(Debug, Default)]
pub struct NullSoeHandler;
impl SoeHandler for NullSoeHandler {}

/// Application-level hooks for a DNP3 master.
#[allow(unused_variables)]
pub trait MasterApplication {
    fn on_receive_iin(&mut self, iin: IinField) {}
    fn on_task_start(&mut self, task_type: MasterTaskType) {}
    fn on_task_complete(&mut self, task_type: MasterTaskType, result: TaskCompletion) {}
    fn now_dnp_time(&self) -> DnpTime {
        DnpTime::invalid()
    }
}

/// Default no-op [`MasterApplication`].
#[derive(Debug, Default)]
pub struct DefaultMasterApplication;
impl MasterApplication for DefaultMasterApplication {}

/// Outcome of a single command point in a command response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPointResult {
    pub header_index: usize,
    pub index: u16,
    pub state: CommandPointState,
    pub status: CommandStatus,
}

/// Overall result of a command operation (SBO or DirectOperate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub summary: TaskCompletion,
    pub points: Vec<CommandPointResult>,
}
