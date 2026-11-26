//! BoltFFI `#[export]` callback traits implemented by Python, C#, Java, etc.

use std::sync::Arc;
use std::time::Duration;

use boltffi::export;
use opendnp3::clock::MonotonicTime;
use opendnp3::driver::AsyncStream;

use crate::types::{
    Analog, AnalogOutputStatus, Binary, BinaryOutputStatus, CommandStatus, Counter, Crob,
    DoubleBitBinary, FrozenCounter, OctetString, OperateType, StreamError,
};

/// Async transport interface implemented in the target language (Python `asyncio`,
/// C# `NetworkStream`/`SerialPort`, Java `NIO`/`Netty`, etc.).
///
/// Represents one live bidirectional byte stream + async timer. Socket lifetime
/// (reconnect loops or server accept loops) stays in the target language.
#[export]
#[async_trait::async_trait]
pub trait Dnp3Stream: Send + Sync {
    async fn read(&self, max_bytes: u32) -> Result<Vec<u8>, StreamError>;
    async fn write(&self, data: Vec<u8>) -> Result<(), StreamError>;
    async fn sleep_ms(&self, duration_ms: u64);
    fn now_ms(&self) -> u64;
}

/// Measurement / SOE callback interface implemented by master users.
#[export]
pub trait SoeHandler: Send + Sync {
    fn on_binary(&self, values: Vec<Binary>);
    fn on_double_bit(&self, values: Vec<DoubleBitBinary>);
    fn on_binary_output_status(&self, values: Vec<BinaryOutputStatus>);
    fn on_counter(&self, values: Vec<Counter>);
    fn on_frozen_counter(&self, values: Vec<FrozenCounter>);
    fn on_analog(&self, values: Vec<Analog>);
    fn on_analog_output_status(&self, values: Vec<AnalogOutputStatus>);
    fn on_octet_string(&self, values: Vec<OctetString>);
}

/// Control command handler interface implemented by outstation users.
#[export]
pub trait CommandHandler: Send + Sync {
    fn select_crob(&self, command: Crob, index: u16) -> CommandStatus;
    fn operate_crob(&self, command: Crob, index: u16, op_type: OperateType) -> CommandStatus;
    fn select_ao_i32(&self, value: i32, index: u16) -> CommandStatus;
    fn operate_ao_i32(&self, value: i32, index: u16, op_type: OperateType) -> CommandStatus;
    fn select_ao_i16(&self, value: i16, index: u16) -> CommandStatus;
    fn operate_ao_i16(&self, value: i16, index: u16, op_type: OperateType) -> CommandStatus;
    fn select_ao_f32(&self, value: f32, index: u16) -> CommandStatus;
    fn operate_ao_f32(&self, value: f32, index: u16, op_type: OperateType) -> CommandStatus;
    fn select_ao_f64(&self, value: f64, index: u16) -> CommandStatus;
    fn operate_ao_f64(&self, value: f64, index: u16, op_type: OperateType) -> CommandStatus;
}

// ---- Internal bridge adapters ----

pub(crate) struct StreamBridge(pub Arc<dyn Dnp3Stream>);

impl AsyncStream for StreamBridge {
    async fn read(&self, max_bytes: usize) -> Result<Vec<u8>, opendnp3::driver::StreamError> {
        self.0
            .read(max_bytes.min(u32::MAX as usize) as u32)
            .await
            .map_err(|e| match e {
                StreamError::Closed => opendnp3::driver::StreamError::Closed,
                StreamError::Io { message } => opendnp3::driver::StreamError::Io(message),
            })
    }

    async fn write(&self, data: &[u8]) -> Result<(), opendnp3::driver::StreamError> {
        self.0.write(data.to_vec()).await.map_err(|e| match e {
            StreamError::Closed => opendnp3::driver::StreamError::Closed,
            StreamError::Io { message } => opendnp3::driver::StreamError::Io(message),
        })
    }

    async fn sleep(&self, duration: Duration) {
        let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        self.0.sleep_ms(ms).await;
    }

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(self.0.now_ms())
    }
}

pub(crate) struct SoeBridge(pub Arc<dyn SoeHandler>);

impl opendnp3::master::SoeHandler for SoeBridge {
    fn process_binary(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::Binary>],
    ) {
        self.0.on_binary(
            values
                .iter()
                .map(|i| Binary::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_double_bit(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::DoubleBitBinary>],
    ) {
        self.0.on_double_bit(
            values
                .iter()
                .map(|i| DoubleBitBinary::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_binary_output_status(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::BinaryOutputStatus>],
    ) {
        self.0.on_binary_output_status(
            values
                .iter()
                .map(|i| BinaryOutputStatus::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_counter(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::Counter>],
    ) {
        self.0.on_counter(
            values
                .iter()
                .map(|i| Counter::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_frozen_counter(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::FrozenCounter>],
    ) {
        self.0.on_frozen_counter(
            values
                .iter()
                .map(|i| FrozenCounter::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_analog(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::Analog>],
    ) {
        self.0.on_analog(
            values
                .iter()
                .map(|i| Analog::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_analog_output_status(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::AnalogOutputStatus>],
    ) {
        self.0.on_analog_output_status(
            values
                .iter()
                .map(|i| AnalogOutputStatus::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_octet_string(
        &mut self,
        _info: opendnp3::app::HeaderInfo,
        values: &[opendnp3::app::Indexed<opendnp3::app::OctetString>],
    ) {
        self.0.on_octet_string(
            values
                .iter()
                .map(|i| OctetString::from_core(i.index, i.value.clone()))
                .collect(),
        );
    }
}

pub(crate) struct CommandBridge(pub Arc<dyn CommandHandler>);

impl opendnp3::outstation::CommandHandler for CommandBridge {
    fn select_crob(
        &mut self,
        command: &opendnp3::app::ControlRelayOutputBlock,
        index: u16,
    ) -> opendnp3::enums::CommandStatus {
        self.0.select_crob((*command).into(), index).into()
    }
    fn operate_crob(
        &mut self,
        command: &opendnp3::app::ControlRelayOutputBlock,
        index: u16,
        op_type: opendnp3::enums::OperateType,
    ) -> opendnp3::enums::CommandStatus {
        self.0
            .operate_crob((*command).into(), index, op_type.into())
            .into()
    }
    fn select_ao_i32(
        &mut self,
        command: &opendnp3::app::AnalogOutputInt32,
        index: u16,
    ) -> opendnp3::enums::CommandStatus {
        self.0.select_ao_i32(command.value, index).into()
    }
    fn operate_ao_i32(
        &mut self,
        command: &opendnp3::app::AnalogOutputInt32,
        index: u16,
        op_type: opendnp3::enums::OperateType,
    ) -> opendnp3::enums::CommandStatus {
        self.0
            .operate_ao_i32(command.value, index, op_type.into())
            .into()
    }
    fn select_ao_i16(
        &mut self,
        command: &opendnp3::app::AnalogOutputInt16,
        index: u16,
    ) -> opendnp3::enums::CommandStatus {
        self.0.select_ao_i16(command.value, index).into()
    }
    fn operate_ao_i16(
        &mut self,
        command: &opendnp3::app::AnalogOutputInt16,
        index: u16,
        op_type: opendnp3::enums::OperateType,
    ) -> opendnp3::enums::CommandStatus {
        self.0
            .operate_ao_i16(command.value, index, op_type.into())
            .into()
    }
    fn select_ao_f32(
        &mut self,
        command: &opendnp3::app::AnalogOutputFloat32,
        index: u16,
    ) -> opendnp3::enums::CommandStatus {
        self.0.select_ao_f32(command.value, index).into()
    }
    fn operate_ao_f32(
        &mut self,
        command: &opendnp3::app::AnalogOutputFloat32,
        index: u16,
        op_type: opendnp3::enums::OperateType,
    ) -> opendnp3::enums::CommandStatus {
        self.0
            .operate_ao_f32(command.value, index, op_type.into())
            .into()
    }
    fn select_ao_f64(
        &mut self,
        command: &opendnp3::app::AnalogOutputDouble64,
        index: u16,
    ) -> opendnp3::enums::CommandStatus {
        self.0.select_ao_f64(command.value, index).into()
    }
    fn operate_ao_f64(
        &mut self,
        command: &opendnp3::app::AnalogOutputDouble64,
        index: u16,
        op_type: opendnp3::enums::OperateType,
    ) -> opendnp3::enums::CommandStatus {
        self.0
            .operate_ao_f64(command.value, index, op_type.into())
            .into()
    }
}
