//! End-to-end Tokio-free integration test exercising the BoltFFI surface
//! (`FfiMaster`, `FfiOutstation`, `Dnp3Stream`, `FfiSoeHandler`, `FfiCommandHandler`).

use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use futures_util::future::join3;
use opendnp3::driver::{UnboundedReceiver, UnboundedSender, unbounded};
use opendnp3_ffi::{
    Dnp3Stream, FfiAnalog, FfiAnalogOutputStatus, FfiBinary, FfiBinaryOutputStatus, FfiClassField,
    FfiCommandHandler, FfiCommandSet, FfiCommandStatus, FfiCounter, FfiCrob, FfiDatabaseConfig,
    FfiDnpTime, FfiDoubleBitBinary, FfiEventMode, FfiFrozenCounter, FfiLinkConfig, FfiMaster,
    FfiMasterEvent, FfiMasterParams, FfiMasterTaskType, FfiOctetString, FfiOperateType,
    FfiOperationType, FfiOutstation, FfiOutstationParams, FfiSoeHandler, FfiStreamError,
    FfiTaskCompletion, FfiTimeSyncMode, FfiTimestampQuality, FfiTripCloseCode,
};

struct ThreadWaker(Thread);
impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => thread::park(),
        }
    }
}

struct MockFfiStream {
    origin: Instant,
    rx: UnboundedReceiver<Vec<u8>>,
    tx: UnboundedSender<Vec<u8>>,
}

#[async_trait::async_trait]
impl Dnp3Stream for MockFfiStream {
    async fn read(&self, _max_bytes: u32) -> Result<Vec<u8>, FfiStreamError> {
        self.rx.recv().await.ok_or(FfiStreamError::Closed)
    }

    async fn write(&self, data: Vec<u8>) -> Result<(), FfiStreamError> {
        self.tx.send(data).map_err(|_| FfiStreamError::Closed)
    }

    async fn sleep_ms(&self, duration_ms: u64) {
        let (tx, rx) = unbounded();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(duration_ms));
            let _ = tx.send(());
        });
        let _ = rx.recv().await;
    }

    fn now_ms(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }
}

#[derive(Default)]
struct MockSoe {
    binaries: Mutex<Vec<FfiBinary>>,
    analogs: Mutex<Vec<FfiAnalog>>,
}

impl FfiSoeHandler for MockSoe {
    fn on_binary(&self, values: Vec<FfiBinary>) {
        self.binaries.lock().unwrap().extend(values);
    }
    fn on_double_bit(&self, _values: Vec<FfiDoubleBitBinary>) {}
    fn on_binary_output_status(&self, _values: Vec<FfiBinaryOutputStatus>) {}
    fn on_counter(&self, _values: Vec<FfiCounter>) {}
    fn on_frozen_counter(&self, _values: Vec<FfiFrozenCounter>) {}
    fn on_analog(&self, values: Vec<FfiAnalog>) {
        self.analogs.lock().unwrap().extend(values);
    }
    fn on_analog_output_status(&self, _values: Vec<FfiAnalogOutputStatus>) {}
    fn on_octet_string(&self, _values: Vec<FfiOctetString>) {}
}

#[derive(Default)]
struct MockCmd {
    operated_crobs: Mutex<Vec<u16>>,
}

impl FfiCommandHandler for MockCmd {
    fn select_crob(&self, _command: FfiCrob, _index: u16) -> FfiCommandStatus {
        FfiCommandStatus::Success
    }
    fn operate_crob(
        &self,
        _command: FfiCrob,
        index: u16,
        _op_type: FfiOperateType,
    ) -> FfiCommandStatus {
        self.operated_crobs.lock().unwrap().push(index);
        FfiCommandStatus::Success
    }
    fn select_ao_i32(&self, _value: i32, _index: u16) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
    fn operate_ao_i32(&self, _value: i32, _index: u16, _op: FfiOperateType) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
    fn select_ao_i16(&self, _value: i16, _index: u16) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
    fn operate_ao_i16(&self, _value: i16, _index: u16, _op: FfiOperateType) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
    fn select_ao_f32(&self, _value: f32, _index: u16) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
    fn operate_ao_f32(&self, _value: f32, _index: u16, _op: FfiOperateType) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
    fn select_ao_f64(&self, _value: f64, _index: u16) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
    fn operate_ao_f64(&self, _value: f64, _index: u16, _op: FfiOperateType) -> FfiCommandStatus {
        FfiCommandStatus::NotSupported
    }
}

#[test]
fn ffi_master_and_outstation_end_to_end() {
    let origin = Instant::now();
    let (a_tx, b_rx) = unbounded();
    let (b_tx, a_rx) = unbounded();
    let m_stream = Arc::new(MockFfiStream {
        origin,
        rx: a_rx,
        tx: a_tx,
    });
    let o_stream = Arc::new(MockFfiStream {
        origin,
        rx: b_rx,
        tx: b_tx,
    });

    let all_classes = FfiClassField {
        class0: true,
        class1: true,
        class2: true,
        class3: true,
    };
    let all_events = FfiClassField {
        class0: false,
        class1: true,
        class2: true,
        class3: true,
    };
    let no_classes = FfiClassField {
        class0: false,
        class1: false,
        class2: false,
        class3: false,
    };

    let db = FfiDatabaseConfig::all_types(3);
    let cmd_handler = Arc::new(MockCmd::default());
    let outstation = Arc::new(FfiOutstation::new(
        FfiLinkConfig {
            is_master: false,
            local_addr: 10,
            remote_addr: 1,
            timeout_ms: 1000,
            keep_alive_timeout_ms: 60_000,
            respond_to_any_source: false,
        },
        FfiOutstationParams {
            max_tx_frag_size: 2048,
            max_rx_frag_size: 2048,
            allow_unsolicited: false,
            sol_confirm_timeout_ms: 5000,
            unsol_confirm_timeout_ms: 5000,
            unsol_retry_delay_ms: 5000,
            select_timeout_ms: 10_000,
            max_controls_per_request: 16,
            unsol_class_mask: all_events,
            respond_to_any_master: false,
            event_buffer_size_per_type: 10,
        },
        &db,
        cmd_handler.clone(),
    ));

    outstation.update_binary(
        FfiBinary {
            index: 0,
            value: true,
            flags: 0x81,
            time: FfiDnpTime {
                value_ms: 0,
                quality: FfiTimestampQuality::Invalid,
            },
        },
        FfiEventMode::Suppress,
    );
    outstation.update_analog(
        FfiAnalog {
            index: 2,
            value: 999.0,
            flags: 0x01,
            time: FfiDnpTime {
                value_ms: 0,
                quality: FfiTimestampQuality::Invalid,
            },
        },
        FfiEventMode::Suppress,
    );

    let soe = Arc::new(MockSoe::default());
    let master = Arc::new(FfiMaster::new(
        FfiLinkConfig {
            is_master: true,
            local_addr: 1,
            remote_addr: 10,
            timeout_ms: 1000,
            keep_alive_timeout_ms: 60_000,
            respond_to_any_source: false,
        },
        FfiMasterParams {
            response_timeout_ms: 5000,
            time_sync_mode: FfiTimeSyncMode::None,
            disable_unsol_on_startup: false,
            ignore_restart_iin: false,
            unsol_class_mask: no_classes,
            startup_integrity_class_mask: all_classes,
            integrity_on_event_overflow_iin: true,
            event_scan_on_events_available_class_mask: no_classes,
            max_tx_frag_size: 2048,
            max_rx_frag_size: 2048,
        },
        soe.clone(),
    ));

    let m_ref = master.clone();
    let o_ref = outstation.clone();
    let driver = async move {
        // Wait for startup integrity poll
        while let Some(ev) = m_ref.next_event().await {
            if matches!(
                ev,
                FfiMasterEvent::TaskComplete {
                    task_type: FfiMasterTaskType::StartupIntegrityPoll,
                    result: FfiTaskCompletion::Success,
                }
            ) {
                break;
            }
        }

        // Issue SBO CROB command via FfiCommandSet
        let cs = FfiCommandSet::new();
        cs.add_crob(
            1,
            FfiCrob {
                op_type: FfiOperationType::PulseOn,
                tcc: FfiTripCloseCode::Close,
                clear: false,
                count: 1,
                on_time_ms: 100,
                off_time_ms: 100,
                status: FfiCommandStatus::Success,
            },
        );
        m_ref.select_and_operate(&cs);

        while let Some(ev) = m_ref.next_event().await {
            if let FfiMasterEvent::CommandComplete { result } = ev {
                assert_eq!(result.summary, FfiTaskCompletion::Success);
                assert_eq!(result.points.len(), 1);
                assert_eq!(result.points[0].status, FfiCommandStatus::Success);
                break;
            }
        }

        m_ref.shutdown();
        o_ref.shutdown();
    };

    let (m_done, _o_done, ()) = block_on(join3(
        master.run(m_stream),
        outstation.run(o_stream),
        driver,
    ));
    assert!(m_done);
    assert!(
        soe.binaries
            .lock()
            .unwrap()
            .iter()
            .any(|b| b.index == 0 && b.value)
    );
    assert!(
        soe.analogs
            .lock()
            .unwrap()
            .iter()
            .any(|a| a.index == 2 && (a.value - 999.0).abs() < 1e-6)
    );
    assert_eq!(*cmd_handler.operated_crobs.lock().unwrap(), vec![1]);
}
