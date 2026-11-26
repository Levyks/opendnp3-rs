//! Exported BoltFFI Outstation & DatabaseConfig classes (100% Tokio-free).

use std::sync::{Arc, Mutex};

use boltffi::export;
use opendnp3::driver::{
    OutstationCommand, OutstationHandle, UnboundedReceiver, outstation_channel,
    run_outstation_session,
};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::outstation::{
    AnalogConfig, AnalogOutputStatusConfig, BinaryConfig, BinaryOutputStatusConfig, CounterConfig,
    DatabaseConfig, DefaultOutstationApplication, DoubleBinaryConfig, FrozenCounterConfig,
    OctetStringConfig, Outstation, OutstationConfig,
};

use crate::callbacks::{CommandBridge, Dnp3Stream, FfiCommandHandler, StreamBridge};
use crate::pipe::{FfiBytePipe, PipeStream};
use crate::types::{
    FfiAnalog, FfiAnalogOutputStatus, FfiBinary, FfiBinaryOutputStatus, FfiCounter,
    FfiDoubleBitBinary, FfiEventClass, FfiEventMode, FfiFrozenCounter, FfiLinkConfig,
    FfiOctetString, FfiOutstationParams,
};

/// Builder for configuring an outstation's static database points.
#[export]
pub struct FfiDatabaseConfig {
    inner: Mutex<DatabaseConfig>,
}

#[export]
impl FfiDatabaseConfig {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(DatabaseConfig::default()),
        }
    }

    pub fn all_types(count: u16) -> Self {
        Self {
            inner: Mutex::new(DatabaseConfig::all_types(count)),
        }
    }

    pub fn add_binary(&self, index: u16, class: Option<FfiEventClass>) {
        let cfg = BinaryConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner.lock().unwrap().binary.insert(index, cfg);
    }

    pub fn add_double_bit(&self, index: u16, class: Option<FfiEventClass>) {
        let cfg = DoubleBinaryConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner.lock().unwrap().double_binary.insert(index, cfg);
    }

    pub fn add_binary_output_status(&self, index: u16, class: Option<FfiEventClass>) {
        let cfg = BinaryOutputStatusConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner
            .lock()
            .unwrap()
            .binary_output_status
            .insert(index, cfg);
    }

    pub fn add_counter(&self, index: u16, class: Option<FfiEventClass>, deadband: f64) {
        let cfg = CounterConfig {
            class: class.map(Into::into),
            deadband,
            ..Default::default()
        };
        self.inner.lock().unwrap().counter.insert(index, cfg);
    }

    pub fn add_frozen_counter(&self, index: u16, class: Option<FfiEventClass>, deadband: f64) {
        let cfg = FrozenCounterConfig {
            class: class.map(Into::into),
            deadband,
            ..Default::default()
        };
        self.inner.lock().unwrap().frozen_counter.insert(index, cfg);
    }

    pub fn add_analog(&self, index: u16, class: Option<FfiEventClass>, deadband: f64) {
        let cfg = AnalogConfig {
            class: class.map(Into::into),
            deadband,
            ..Default::default()
        };
        self.inner.lock().unwrap().analog.insert(index, cfg);
    }

    pub fn add_analog_output_status(
        &self,
        index: u16,
        class: Option<FfiEventClass>,
        deadband: f64,
    ) {
        let cfg = AnalogOutputStatusConfig {
            class: class.map(Into::into),
            deadband,
            ..Default::default()
        };
        self.inner
            .lock()
            .unwrap()
            .analog_output_status
            .insert(index, cfg);
    }

    pub fn add_octet_string(&self, index: u16, class: Option<FfiEventClass>) {
        let cfg = OctetStringConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner.lock().unwrap().octet_string.insert(index, cfg);
    }
}

impl Default for FfiDatabaseConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful DNP3 Outstation driven by the target language's async runtime.
#[export]
pub struct FfiOutstation {
    link_cfg: LinkConfig,
    outstation: Mutex<Option<Outstation>>,
    cmd_handler: Arc<dyn FfiCommandHandler>,
    handle: OutstationHandle,
    cmd_rx: UnboundedReceiver<OutstationCommand>,
}

struct RestoreOnDrop<'a, T> {
    slot: &'a Mutex<Option<T>>,
    value: Option<T>,
}

impl<T> Drop for RestoreOnDrop<'_, T> {
    fn drop(&mut self) {
        if let Some(v) = self.value.take() {
            *self.slot.lock().unwrap() = Some(v);
        }
    }
}

#[export]
impl FfiOutstation {
    pub fn new(
        link_cfg: FfiLinkConfig,
        params: FfiOutstationParams,
        database: &FfiDatabaseConfig,
        cmd_handler: Arc<dyn FfiCommandHandler>,
    ) -> Self {
        let link_cfg: LinkConfig = link_cfg.into();
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let (o_params, event_buffer) = params.into_core();
        let cfg = OutstationConfig {
            params: o_params,
            event_buffer,
            database: database.inner.lock().unwrap().clone(),
        };
        let outstation = Outstation::new(cfg, addresses);
        let (handle, cmd_rx) = outstation_channel();
        Self {
            link_cfg,
            outstation: Mutex::new(Some(outstation)),
            cmd_handler,
            handle,
            cmd_rx,
        }
    }

    /// Drive a single connected [`Dnp3Stream`] until EOF, error, or [`shutdown`](Self::shutdown).
    /// Returns `true` if `shutdown()` was called, `false` if the stream closed.
    pub async fn run(&self, stream: Arc<dyn Dnp3Stream>) -> bool {
        let taken = self.outstation.lock().unwrap().take();
        let Some(outstation) = taken else {
            return false;
        };
        let mut guard = RestoreOnDrop {
            slot: &self.outstation,
            value: Some(outstation),
        };
        let bridge = StreamBridge(stream);
        let mut cmd_handler = CommandBridge(Arc::clone(&self.cmd_handler));
        let mut app = DefaultOutstationApplication;
        run_outstation_session(
            &bridge,
            self.link_cfg.clone(),
            guard.value.as_mut().unwrap(),
            &mut cmd_handler,
            &mut app,
            &self.cmd_rx,
        )
        .await
    }

    /// Drive a single connection using an [`FfiBytePipe`] (recommended for Python `asyncio`).
    /// Returns `true` if `shutdown()` was called, `false` if the pipe closed.
    pub async fn run_with_pipe(&self, pipe: &FfiBytePipe) -> bool {
        let taken = self.outstation.lock().unwrap().take();
        let Some(outstation) = taken else {
            return false;
        };
        let mut guard = RestoreOnDrop {
            slot: &self.outstation,
            value: Some(outstation),
        };
        let stream = PipeStream::new(pipe);
        let mut cmd_handler = CommandBridge(Arc::clone(&self.cmd_handler));
        let mut app = DefaultOutstationApplication;
        run_outstation_session(
            &stream,
            self.link_cfg.clone(),
            guard.value.as_mut().unwrap(),
            &mut cmd_handler,
            &mut app,
            &self.cmd_rx,
        )
        .await
    }

    pub fn update_binary(&self, value: FfiBinary, mode: FfiEventMode) {
        self.handle
            .update_binary(value.index, value.into_core(), mode.into());
    }

    pub fn update_double_bit(&self, value: FfiDoubleBitBinary, mode: FfiEventMode) {
        self.handle
            .update_double_bit(value.index, value.into_core(), mode.into());
    }

    pub fn update_binary_output_status(&self, value: FfiBinaryOutputStatus, mode: FfiEventMode) {
        self.handle
            .update_binary_output_status(value.index, value.into_core(), mode.into());
    }

    pub fn update_counter(&self, value: FfiCounter, mode: FfiEventMode) {
        self.handle
            .update_counter(value.index, value.into_core(), mode.into());
    }

    pub fn update_frozen_counter(&self, value: FfiFrozenCounter, mode: FfiEventMode) {
        self.handle
            .update_frozen_counter(value.index, value.into_core(), mode.into());
    }

    pub fn update_analog(&self, value: FfiAnalog, mode: FfiEventMode) {
        self.handle
            .update_analog(value.index, value.into_core(), mode.into());
    }

    pub fn update_analog_output_status(&self, value: FfiAnalogOutputStatus, mode: FfiEventMode) {
        self.handle
            .update_analog_output_status(value.index, value.into_core(), mode.into());
    }

    pub fn update_octet_string(&self, value: FfiOctetString, mode: FfiEventMode) {
        self.handle
            .update_octet_string(value.index, value.into_core(), mode.into());
    }

    pub fn shutdown(&self) {
        self.handle.shutdown();
    }
}
