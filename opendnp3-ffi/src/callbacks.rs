//! BoltFFI `#[export]` callback traits implemented by Python, C#, Java, etc.

use std::sync::Arc;
use std::time::Duration;

use boltffi::export;
use opendnp3::app::{
    Analog, AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    AnalogOutputStatus, Binary, BinaryOutputStatus, ControlRelayOutputBlock, Counter,
    DoubleBitBinary, FrozenCounter, HeaderInfo, Indexed, OctetString,
};
use opendnp3::clock::MonotonicTime;
use opendnp3::driver::{AsyncStream, StreamError};
use opendnp3::enums::{CommandStatus, OperateType};
use opendnp3::master::SoeHandler;
use opendnp3::outstation::CommandHandler;

use crate::types::{
    FfiAnalog, FfiAnalogOutputStatus, FfiBinary, FfiBinaryOutputStatus, FfiCommandStatus,
    FfiCounter, FfiCrob, FfiDoubleBitBinary, FfiFrozenCounter, FfiOctetString, FfiOperateType,
    FfiStreamError,
};

/// Async transport interface implemented in the target language (Python `asyncio`,
/// C# `NetworkStream`/`SerialPort`, Java `NIO`/`Netty`, etc.).
///
/// Represents one live bidirectional byte stream + async timer. Socket lifetime
/// (reconnect loops or server accept loops) stays in the target language.
#[export]
#[async_trait::async_trait]
pub trait Dnp3Stream: Send + Sync {
    async fn read(&self, max_bytes: u32) -> Result<Vec<u8>, FfiStreamError>;
    async fn write(&self, data: Vec<u8>) -> Result<(), FfiStreamError>;
    async fn sleep_ms(&self, duration_ms: u64);
    fn now_ms(&self) -> u64;
}

/// Measurement / SOE callback interface implemented by Master users.
#[export]
pub trait FfiSoeHandler: Send + Sync {
    fn on_binary(&self, values: Vec<FfiBinary>);
    fn on_double_bit(&self, values: Vec<FfiDoubleBitBinary>);
    fn on_binary_output_status(&self, values: Vec<FfiBinaryOutputStatus>);
    fn on_counter(&self, values: Vec<FfiCounter>);
    fn on_frozen_counter(&self, values: Vec<FfiFrozenCounter>);
    fn on_analog(&self, values: Vec<FfiAnalog>);
    fn on_analog_output_status(&self, values: Vec<FfiAnalogOutputStatus>);
    fn on_octet_string(&self, values: Vec<FfiOctetString>);
}

/// Control command handler interface implemented by Outstation users.
#[export]
pub trait FfiCommandHandler: Send + Sync {
    fn select_crob(&self, command: FfiCrob, index: u16) -> FfiCommandStatus;
    fn operate_crob(
        &self,
        command: FfiCrob,
        index: u16,
        op_type: FfiOperateType,
    ) -> FfiCommandStatus;
    fn select_ao_i32(&self, value: i32, index: u16) -> FfiCommandStatus;
    fn operate_ao_i32(&self, value: i32, index: u16, op_type: FfiOperateType) -> FfiCommandStatus;
    fn select_ao_i16(&self, value: i16, index: u16) -> FfiCommandStatus;
    fn operate_ao_i16(&self, value: i16, index: u16, op_type: FfiOperateType) -> FfiCommandStatus;
    fn select_ao_f32(&self, value: f32, index: u16) -> FfiCommandStatus;
    fn operate_ao_f32(&self, value: f32, index: u16, op_type: FfiOperateType) -> FfiCommandStatus;
    fn select_ao_f64(&self, value: f64, index: u16) -> FfiCommandStatus;
    fn operate_ao_f64(&self, value: f64, index: u16, op_type: FfiOperateType) -> FfiCommandStatus;
}

// ---- Internal bridge adapters ----

pub(crate) struct StreamBridge(pub Arc<dyn Dnp3Stream>);

impl AsyncStream for StreamBridge {
    async fn read(&self, max_bytes: usize) -> Result<Vec<u8>, StreamError> {
        self.0
            .read(max_bytes.min(u32::MAX as usize) as u32)
            .await
            .map_err(|e| match e {
                FfiStreamError::Closed => StreamError::Closed,
                FfiStreamError::Io { message } => StreamError::Io(message),
            })
    }

    async fn write(&self, data: &[u8]) -> Result<(), StreamError> {
        self.0.write(data.to_vec()).await.map_err(|e| match e {
            FfiStreamError::Closed => StreamError::Closed,
            FfiStreamError::Io { message } => StreamError::Io(message),
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

pub(crate) struct SoeBridge(pub Arc<dyn FfiSoeHandler>);

impl SoeHandler for SoeBridge {
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.0.on_binary(
            values
                .iter()
                .map(|i| FfiBinary::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_double_bit(&mut self, _info: HeaderInfo, values: &[Indexed<DoubleBitBinary>]) {
        self.0.on_double_bit(
            values
                .iter()
                .map(|i| FfiDoubleBitBinary::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_binary_output_status(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<BinaryOutputStatus>],
    ) {
        self.0.on_binary_output_status(
            values
                .iter()
                .map(|i| FfiBinaryOutputStatus::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_counter(&mut self, _info: HeaderInfo, values: &[Indexed<Counter>]) {
        self.0.on_counter(
            values
                .iter()
                .map(|i| FfiCounter::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_frozen_counter(&mut self, _info: HeaderInfo, values: &[Indexed<FrozenCounter>]) {
        self.0.on_frozen_counter(
            values
                .iter()
                .map(|i| FfiFrozenCounter::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_analog(&mut self, _info: HeaderInfo, values: &[Indexed<Analog>]) {
        self.0.on_analog(
            values
                .iter()
                .map(|i| FfiAnalog::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_analog_output_status(
        &mut self,
        _info: HeaderInfo,
        values: &[Indexed<AnalogOutputStatus>],
    ) {
        self.0.on_analog_output_status(
            values
                .iter()
                .map(|i| FfiAnalogOutputStatus::from_core(i.index, i.value))
                .collect(),
        );
    }
    fn process_octet_string(&mut self, _info: HeaderInfo, values: &[Indexed<OctetString>]) {
        self.0.on_octet_string(
            values
                .iter()
                .map(|i| FfiOctetString::from_core(i.index, i.value.clone()))
                .collect(),
        );
    }
}

pub(crate) struct CommandBridge(pub Arc<dyn FfiCommandHandler>);

impl CommandHandler for CommandBridge {
    fn select_crob(&mut self, command: &ControlRelayOutputBlock, index: u16) -> CommandStatus {
        self.0.select_crob((*command).into(), index).into()
    }
    fn operate_crob(
        &mut self,
        command: &ControlRelayOutputBlock,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        self.0
            .operate_crob((*command).into(), index, op_type.into())
            .into()
    }
    fn select_ao_i32(&mut self, command: &AnalogOutputInt32, index: u16) -> CommandStatus {
        self.0.select_ao_i32(command.value, index).into()
    }
    fn operate_ao_i32(
        &mut self,
        command: &AnalogOutputInt32,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        self.0
            .operate_ao_i32(command.value, index, op_type.into())
            .into()
    }
    fn select_ao_i16(&mut self, command: &AnalogOutputInt16, index: u16) -> CommandStatus {
        self.0.select_ao_i16(command.value, index).into()
    }
    fn operate_ao_i16(
        &mut self,
        command: &AnalogOutputInt16,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        self.0
            .operate_ao_i16(command.value, index, op_type.into())
            .into()
    }
    fn select_ao_f32(&mut self, command: &AnalogOutputFloat32, index: u16) -> CommandStatus {
        self.0.select_ao_f32(command.value, index).into()
    }
    fn operate_ao_f32(
        &mut self,
        command: &AnalogOutputFloat32,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        self.0
            .operate_ao_f32(command.value, index, op_type.into())
            .into()
    }
    fn select_ao_f64(&mut self, command: &AnalogOutputDouble64, index: u16) -> CommandStatus {
        self.0.select_ao_f64(command.value, index).into()
    }
    fn operate_ao_f64(
        &mut self,
        command: &AnalogOutputDouble64,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        self.0
            .operate_ao_f64(command.value, index, op_type.into())
            .into()
    }
}
