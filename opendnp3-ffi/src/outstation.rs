//! Exported BoltFFI outstation and database-config classes (Tokio-free).

use std::sync::{Arc, Mutex};

use crate::callbacks::{CommandBridge, CommandHandler, Dnp3Stream, StreamBridge};
use crate::pipe::{BytePipe, PipeStream};
use crate::types::{
    Analog, AnalogOutputStatus, Binary, BinaryOutputStatus, Counter, DoubleBitBinary, EventClass,
    EventMode, FrozenCounter, LinkConfig, OctetString, OutstationParams,
};
use boltffi::export;
use opendnp3::driver::{
    OutstationCommand, OutstationHandle, UnboundedReceiver, outstation_channel,
    run_outstation_session,
};

/// Builder for configuring an outstation's static database points.
#[export]
pub struct DatabaseConfig {
    inner: Mutex<opendnp3::outstation::DatabaseConfig>,
}

#[export]
impl DatabaseConfig {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(opendnp3::outstation::DatabaseConfig::default()),
        }
    }

    pub fn all_types(count: u16) -> Self {
        Self {
            inner: Mutex::new(opendnp3::outstation::DatabaseConfig::all_types(count)),
        }
    }

    pub fn add_binary(&self, index: u16, class: Option<EventClass>) {
        let cfg = opendnp3::outstation::BinaryConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner.lock().unwrap().binary.insert(index, cfg);
    }

    pub fn add_double_bit(&self, index: u16, class: Option<EventClass>) {
        let cfg = opendnp3::outstation::DoubleBinaryConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner.lock().unwrap().double_binary.insert(index, cfg);
    }

    pub fn add_binary_output_status(&self, index: u16, class: Option<EventClass>) {
        let cfg = opendnp3::outstation::BinaryOutputStatusConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner
            .lock()
            .unwrap()
            .binary_output_status
            .insert(index, cfg);
    }

    pub fn add_counter(&self, index: u16, class: Option<EventClass>, deadband: f64) {
        let cfg = opendnp3::outstation::CounterConfig {
            class: class.map(Into::into),
            deadband,
            ..Default::default()
        };
        self.inner.lock().unwrap().counter.insert(index, cfg);
    }

    pub fn add_frozen_counter(&self, index: u16, class: Option<EventClass>, deadband: f64) {
        let cfg = opendnp3::outstation::FrozenCounterConfig {
            class: class.map(Into::into),
            deadband,
            ..Default::default()
        };
        self.inner.lock().unwrap().frozen_counter.insert(index, cfg);
    }

    pub fn add_analog(&self, index: u16, class: Option<EventClass>, deadband: f64) {
        let cfg = opendnp3::outstation::AnalogConfig {
            class: class.map(Into::into),
            deadband,
            ..Default::default()
        };
        self.inner.lock().unwrap().analog.insert(index, cfg);
    }

    pub fn add_analog_output_status(&self, index: u16, class: Option<EventClass>, deadband: f64) {
        let cfg = opendnp3::outstation::AnalogOutputStatusConfig {
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

    pub fn add_octet_string(&self, index: u16, class: Option<EventClass>) {
        let cfg = opendnp3::outstation::OctetStringConfig {
            class: class.map(Into::into),
            ..Default::default()
        };
        self.inner.lock().unwrap().octet_string.insert(index, cfg);
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful DNP3 outstation driven by the target language's async runtime.
#[export]
pub struct Outstation {
    link_cfg: opendnp3::link::LinkConfig,
    outstation: Mutex<Option<opendnp3::outstation::Outstation>>,
    cmd_handler: Arc<dyn CommandHandler>,
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
impl Outstation {
    pub fn new(
        link_cfg: LinkConfig,
        params: OutstationParams,
        database: &DatabaseConfig,
        cmd_handler: Arc<dyn CommandHandler>,
    ) -> Self {
        let link_cfg: opendnp3::link::LinkConfig = link_cfg.into();
        let addresses = opendnp3::link::Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let (o_params, event_buffer) = params.into_core();
        let cfg = opendnp3::outstation::OutstationConfig {
            params: o_params,
            event_buffer,
            database: database.inner.lock().unwrap().clone(),
        };
        let outstation = opendnp3::outstation::Outstation::new(cfg, addresses);
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
        let mut app = opendnp3::outstation::DefaultOutstationApplication;
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

    /// Drive a single connection using an [`BytePipe`] (recommended for Python `asyncio`).
    /// Returns `true` if `shutdown()` was called, `false` if the pipe closed.
    pub async fn run_with_pipe(&self, pipe: &BytePipe) -> bool {
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
        let mut app = opendnp3::outstation::DefaultOutstationApplication;
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

    pub fn update_binary(&self, value: Binary, mode: EventMode) {
        self.handle
            .update_binary(value.index, value.into_core(), mode.into());
    }

    pub fn update_double_bit(&self, value: DoubleBitBinary, mode: EventMode) {
        self.handle
            .update_double_bit(value.index, value.into_core(), mode.into());
    }

    pub fn update_binary_output_status(&self, value: BinaryOutputStatus, mode: EventMode) {
        self.handle
            .update_binary_output_status(value.index, value.into_core(), mode.into());
    }

    pub fn update_counter(&self, value: Counter, mode: EventMode) {
        self.handle
            .update_counter(value.index, value.into_core(), mode.into());
    }

    pub fn update_frozen_counter(&self, value: FrozenCounter, mode: EventMode) {
        self.handle
            .update_frozen_counter(value.index, value.into_core(), mode.into());
    }

    pub fn update_analog(&self, value: Analog, mode: EventMode) {
        self.handle
            .update_analog(value.index, value.into_core(), mode.into());
    }

    pub fn update_analog_output_status(&self, value: AnalogOutputStatus, mode: EventMode) {
        self.handle
            .update_analog_output_status(value.index, value.into_core(), mode.into());
    }

    pub fn update_octet_string(&self, value: OctetString, mode: EventMode) {
        self.handle
            .update_octet_string(value.index, value.into_core(), mode.into());
    }

    pub fn shutdown(&self) {
        self.handle.shutdown();
    }
}
