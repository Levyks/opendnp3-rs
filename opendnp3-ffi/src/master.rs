//! Exported BoltFFI Master & CommandSet classes (100% Tokio-free).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use boltffi::export;
use opendnp3::driver::{
    MasterCommand, MasterHandle, UnboundedReceiver, UnboundedSender, master_channel,
    run_master_session, unbounded,
};
use opendnp3::link::{Addresses, LinkConfig};
use opendnp3::master::{CommandSet, DefaultMasterApplication, Master, MasterEvent};

use crate::callbacks::{Dnp3Stream, FfiSoeHandler, SoeBridge, StreamBridge};
use crate::pipe::{FfiBytePipe, PipeStream};
use crate::types::{
    FfiClassField, FfiCrob, FfiLinkConfig, FfiMasterEvent, FfiMasterParams, FfiPointClass,
    FfiRestartType, ao_f32, ao_f64, ao_i16, ao_i32, gv_from_raw, range_from_raw,
};

/// Mutable builder for CROB and Analog Output commands.
#[export]
pub struct FfiCommandSet {
    inner: Mutex<CommandSet>,
}

#[export]
impl FfiCommandSet {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(CommandSet::new()),
        }
    }

    pub fn add_crob(&self, index: u16, cmd: FfiCrob) {
        self.inner.lock().unwrap().add_crob(index, cmd.into());
    }

    pub fn add_ao_i32(&self, index: u16, value: i32) {
        self.inner.lock().unwrap().add_ao_i32(index, ao_i32(value));
    }

    pub fn add_ao_i16(&self, index: u16, value: i16) {
        self.inner.lock().unwrap().add_ao_i16(index, ao_i16(value));
    }

    pub fn add_ao_f32(&self, index: u16, value: f32) {
        self.inner.lock().unwrap().add_ao_f32(index, ao_f32(value));
    }

    pub fn add_ao_f64(&self, index: u16, value: f64) {
        self.inner.lock().unwrap().add_ao_f64(index, ao_f64(value));
    }
}

impl Default for FfiCommandSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful DNP3 Master driven by the target language's async runtime.
#[export]
pub struct FfiMaster {
    link_cfg: LinkConfig,
    master: Mutex<Option<Master>>,
    soe: Arc<dyn FfiSoeHandler>,
    handle: MasterHandle,
    cmd_rx: UnboundedReceiver<MasterCommand>,
    ev_tx: UnboundedSender<MasterEvent>,
    ev_rx: UnboundedReceiver<MasterEvent>,
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
impl FfiMaster {
    pub fn new(
        link_cfg: FfiLinkConfig,
        params: FfiMasterParams,
        soe: Arc<dyn FfiSoeHandler>,
    ) -> Self {
        let link_cfg: LinkConfig = link_cfg.into();
        let addresses = Addresses::new(link_cfg.local_addr, link_cfg.remote_addr);
        let master = Master::new(params.into(), addresses);
        let (handle, cmd_rx) = master_channel();
        let (ev_tx, ev_rx) = unbounded();
        Self {
            link_cfg,
            master: Mutex::new(Some(master)),
            soe,
            handle,
            cmd_rx,
            ev_tx,
            ev_rx,
        }
    }

    /// Drive a single connected [`Dnp3Stream`] until EOF, error, or [`shutdown`](Self::shutdown).
    /// Returns `true` if `shutdown()` was called, `false` if the stream closed.
    pub async fn run(&self, stream: Arc<dyn Dnp3Stream>) -> bool {
        let taken = self.master.lock().unwrap().take();
        let Some(master) = taken else {
            return false;
        };
        let mut guard = RestoreOnDrop {
            slot: &self.master,
            value: Some(master),
        };
        let bridge = StreamBridge(stream);
        let mut soe = SoeBridge(Arc::clone(&self.soe));
        let mut app = DefaultMasterApplication;
        run_master_session(
            &bridge,
            self.link_cfg.clone(),
            guard.value.as_mut().unwrap(),
            &mut soe,
            &mut app,
            &self.cmd_rx,
            &self.ev_tx,
        )
        .await
    }

    /// Drive a single connection using an [`FfiBytePipe`] (recommended for Python `asyncio`).
    /// Returns `true` if `shutdown()` was called, `false` if the pipe closed.
    pub async fn run_with_pipe(&self, pipe: &FfiBytePipe) -> bool {
        let taken = self.master.lock().unwrap().take();
        let Some(master) = taken else {
            return false;
        };
        let mut guard = RestoreOnDrop {
            slot: &self.master,
            value: Some(master),
        };
        let stream = PipeStream::new(pipe);
        let mut soe = SoeBridge(Arc::clone(&self.soe));
        let mut app = DefaultMasterApplication;
        run_master_session(
            &stream,
            self.link_cfg.clone(),
            guard.value.as_mut().unwrap(),
            &mut soe,
            &mut app,
            &self.cmd_rx,
            &self.ev_tx,
        )
        .await
    }

    /// Await the next [`FfiMasterEvent`] produced by the master.
    pub async fn next_event(&self) -> Option<FfiMasterEvent> {
        self.ev_rx.recv().await.map(Into::into)
    }

    pub fn scan_classes(&self, classes: FfiClassField) {
        self.handle.scan_classes(classes.into());
    }

    pub fn scan_range(&self, group: u8, variation: u8, start: u16, stop: u16) {
        self.handle
            .scan_range(gv_from_raw(group, variation), range_from_raw(start, stop));
    }

    pub fn scan_all_objects(&self, group: u8, variation: u8) {
        self.handle.scan_all_objects(gv_from_raw(group, variation));
    }

    pub fn add_class_scan(&self, classes: FfiClassField, period_ms: u64) {
        self.handle
            .add_class_scan(classes.into(), Duration::from_millis(period_ms));
    }

    pub fn assign_class_all(&self, class: FfiPointClass, group: u8, variation: u8) {
        self.handle
            .assign_class_all(class.into(), gv_from_raw(group, variation));
    }

    pub fn select_and_operate(&self, commands: &FfiCommandSet) {
        let cs = commands.inner.lock().unwrap().clone();
        self.handle.select_and_operate(cs);
    }

    pub fn direct_operate(&self, commands: &FfiCommandSet, no_ack: bool) {
        let cs = commands.inner.lock().unwrap().clone();
        self.handle.direct_operate(cs, no_ack);
    }

    pub fn restart(&self, restart_type: FfiRestartType) {
        self.handle.restart(restart_type.into());
    }

    pub fn enable_unsolicited(&self, classes: FfiClassField) {
        self.handle.enable_unsolicited(classes.into());
    }

    pub fn disable_unsolicited(&self, classes: FfiClassField) {
        self.handle.disable_unsolicited(classes.into());
    }

    pub fn shutdown(&self) {
        self.handle.shutdown();
    }
}
