//! Outstation user callback traits (`CommandHandler` and `OutstationApplication`).

use crate::app::{
    AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    ControlRelayOutputBlock, DnpTime, IinField, Range,
};
use crate::enums::{CommandStatus, EventClass, OperateType, RestartMode};

/// Handles control commands (CROB and Analog Outputs) received by an outstation.
#[allow(unused_variables)]
pub trait CommandHandler {
    fn begin(&mut self) {}
    fn end(&mut self) {}

    fn select_crob(&mut self, command: &ControlRelayOutputBlock, index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_crob(
        &mut self,
        command: &ControlRelayOutputBlock,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }

    fn select_ao_i32(&mut self, command: &AnalogOutputInt32, index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_i32(
        &mut self,
        command: &AnalogOutputInt32,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }

    fn select_ao_i16(&mut self, command: &AnalogOutputInt16, index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_i16(
        &mut self,
        command: &AnalogOutputInt16,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }

    fn select_ao_f32(&mut self, command: &AnalogOutputFloat32, index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_f32(
        &mut self,
        command: &AnalogOutputFloat32,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }

    fn select_ao_f64(&mut self, command: &AnalogOutputDouble64, index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_f64(
        &mut self,
        command: &AnalogOutputDouble64,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        CommandStatus::NotSupported
    }
}

/// Default command handler that rejects all commands with `NotSupported`.
#[derive(Debug, Default)]
pub struct DefaultCommandHandler;
impl CommandHandler for DefaultCommandHandler {}

/// Application-level hooks for outstation IINs, time sync, and restarts.
#[allow(unused_variables)]
pub trait OutstationApplication {
    fn application_iin(&self) -> IinField {
        IinField::empty()
    }

    fn supports_write_absolute_time(&self) -> bool {
        false
    }

    fn write_absolute_time(&mut self, time: DnpTime) -> bool {
        false
    }

    fn supports_assign_class(&self) -> bool {
        false
    }

    fn record_class_assignment(&mut self, group: u8, class: Option<EventClass>, range: Range) {}

    fn cold_restart_support(&self) -> RestartMode {
        RestartMode::Unsupported
    }

    fn warm_restart_support(&self) -> RestartMode {
        RestartMode::Unsupported
    }

    fn cold_restart(&mut self) -> u16 {
        u16::MAX
    }

    fn warm_restart(&mut self) -> u16 {
        u16::MAX
    }

    fn now_dnp_time(&self) -> DnpTime {
        DnpTime::invalid()
    }
}

/// Default outstation application with no optional features enabled.
#[derive(Debug, Default)]
pub struct DefaultOutstationApplication;
impl OutstationApplication for DefaultOutstationApplication {}
