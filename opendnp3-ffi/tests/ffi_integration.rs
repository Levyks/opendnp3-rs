//! End-to-end Tokio-free integration test exercising the BoltFFI surface
//! (`Master`, `Outstation`, `Dnp3Stream`, `SoeHandler`, `CommandHandler`).

use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use futures_util::future::join3;
use opendnp3::driver::{UnboundedReceiver, UnboundedSender, unbounded};
use opendnp3_ffi::{
    Analog, AnalogOutputStatus, Binary, BinaryOutputStatus, ClassField, CommandHandler, CommandSet,
    CommandStatus, Counter, Crob, DatabaseConfig, Dnp3Stream, DnpTime, DoubleBitBinary, EventMode,
    FrozenCounter, LinkConfig, Master, MasterEvent, MasterParams, MasterTaskType, OctetString,
    OperateType, OperationType, Outstation, OutstationParams, SoeHandler, StreamError,
    TaskCompletion, TimeSyncMode, TimestampQuality, TripCloseCode,
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

struct MockStream {
    origin: Instant,
    rx: UnboundedReceiver<Vec<u8>>,
    tx: UnboundedSender<Vec<u8>>,
}

#[async_trait::async_trait]
impl Dnp3Stream for MockStream {
    async fn read(&self, _max_bytes: u32) -> Result<Vec<u8>, StreamError> {
        self.rx.recv().await.ok_or(StreamError::Closed)
    }

    async fn write(&self, data: Vec<u8>) -> Result<(), StreamError> {
        self.tx.send(data).map_err(|_| StreamError::Closed)
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
    binaries: Mutex<Vec<Binary>>,
    analogs: Mutex<Vec<Analog>>,
}

impl SoeHandler for MockSoe {
    fn on_binary(&self, values: Vec<Binary>) {
        self.binaries.lock().unwrap().extend(values);
    }
    fn on_double_bit(&self, _values: Vec<DoubleBitBinary>) {}
    fn on_binary_output_status(&self, _values: Vec<BinaryOutputStatus>) {}
    fn on_counter(&self, _values: Vec<Counter>) {}
    fn on_frozen_counter(&self, _values: Vec<FrozenCounter>) {}
    fn on_analog(&self, values: Vec<Analog>) {
        self.analogs.lock().unwrap().extend(values);
    }
    fn on_analog_output_status(&self, _values: Vec<AnalogOutputStatus>) {}
    fn on_octet_string(&self, _values: Vec<OctetString>) {}
}

#[derive(Default)]
struct MockCmd {
    operated_crobs: Mutex<Vec<u16>>,
}

impl CommandHandler for MockCmd {
    fn select_crob(&self, _command: Crob, _index: u16) -> CommandStatus {
        CommandStatus::Success
    }
    fn operate_crob(&self, _command: Crob, index: u16, _op_type: OperateType) -> CommandStatus {
        self.operated_crobs.lock().unwrap().push(index);
        CommandStatus::Success
    }
    fn select_ao_i32(&self, _value: i32, _index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_i32(&self, _value: i32, _index: u16, _op: OperateType) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn select_ao_i16(&self, _value: i16, _index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_i16(&self, _value: i16, _index: u16, _op: OperateType) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn select_ao_f32(&self, _value: f32, _index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_f32(&self, _value: f32, _index: u16, _op: OperateType) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn select_ao_f64(&self, _value: f64, _index: u16) -> CommandStatus {
        CommandStatus::NotSupported
    }
    fn operate_ao_f64(&self, _value: f64, _index: u16, _op: OperateType) -> CommandStatus {
        CommandStatus::NotSupported
    }
}

#[test]
fn ffi_master_and_outstation_end_to_end() {
    let origin = Instant::now();
    let (a_tx, b_rx) = unbounded();
    let (b_tx, a_rx) = unbounded();
    let m_stream = Arc::new(MockStream {
        origin,
        rx: a_rx,
        tx: a_tx,
    });
    let o_stream = Arc::new(MockStream {
        origin,
        rx: b_rx,
        tx: b_tx,
    });

    let all_classes = ClassField {
        class0: true,
        class1: true,
        class2: true,
        class3: true,
    };
    let all_events = ClassField {
        class0: false,
        class1: true,
        class2: true,
        class3: true,
    };
    let no_classes = ClassField {
        class0: false,
        class1: false,
        class2: false,
        class3: false,
    };

    let db = DatabaseConfig::all_types(3);
    let cmd_handler = Arc::new(MockCmd::default());
    let outstation = Arc::new(Outstation::new(
        LinkConfig {
            is_master: false,
            local_addr: 10,
            remote_addr: 1,
            timeout_ms: 1000,
            keep_alive_timeout_ms: 60_000,
            respond_to_any_source: false,
        },
        OutstationParams {
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
        Binary {
            index: 0,
            value: true,
            flags: 0x81,
            time: DnpTime {
                value_ms: 0,
                quality: TimestampQuality::Invalid,
            },
        },
        EventMode::Suppress,
    );
    outstation.update_analog(
        Analog {
            index: 2,
            value: 999.0,
            flags: 0x01,
            time: DnpTime {
                value_ms: 0,
                quality: TimestampQuality::Invalid,
            },
        },
        EventMode::Suppress,
    );

    let soe = Arc::new(MockSoe::default());
    let master = Arc::new(Master::new(
        LinkConfig {
            is_master: true,
            local_addr: 1,
            remote_addr: 10,
            timeout_ms: 1000,
            keep_alive_timeout_ms: 60_000,
            respond_to_any_source: false,
        },
        MasterParams {
            response_timeout_ms: 5000,
            time_sync_mode: TimeSyncMode::None,
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
                MasterEvent::TaskComplete {
                    task_type: MasterTaskType::StartupIntegrityPoll,
                    result: TaskCompletion::Success,
                }
            ) {
                break;
            }
        }

        // Issue SBO CROB command via CommandSet
        let cs = CommandSet::new();
        cs.add_crob(
            1,
            Crob {
                op_type: OperationType::PulseOn,
                tcc: TripCloseCode::Close,
                clear: false,
                count: 1,
                on_time_ms: 100,
                off_time_ms: 100,
                status: CommandStatus::Success,
            },
        );
        m_ref.select_and_operate(&cs);

        while let Some(ev) = m_ref.next_event().await {
            if let MasterEvent::CommandComplete { result } = ev {
                assert_eq!(result.summary, TaskCompletion::Success);
                assert_eq!(result.points.len(), 1);
                assert_eq!(result.points[0].status, CommandStatus::Success);
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
