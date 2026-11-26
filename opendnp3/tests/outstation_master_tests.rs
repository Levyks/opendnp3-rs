//! Unit and protocol-loop tests for Outstation and Master state machines.

use std::time::Duration;

use opendnp3::Message;
use opendnp3::app::{
    Analog, AnalogOutputInt32, Binary, ClassField, ControlRelayOutputBlock, Counter, DnpTime,
    HeaderInfo, IinBit, IinField, Indexed, Range,
};
use opendnp3::clock::ManualClock;
use opendnp3::enums::{
    CommandStatus, EventMode, GroupVariation, OperateType, OperationType, RestartMode, RestartType,
    TaskCompletion, TripCloseCode,
};
use opendnp3::link::{Addresses, FrameParser, LinkConfig, LinkLayer};
use opendnp3::master::{
    CommandSet, DefaultMasterApplication, Master, MasterEvent, MasterParams, ResponseInfo,
    SoeHandler,
};
use opendnp3::outstation::{
    CommandHandler, DatabaseConfig, DefaultOutstationApplication, EventBufferConfig, Outstation,
    OutstationApplication, OutstationConfig,
};
use opendnp3::transport::{TransportEvent, TransportLayer};

#[derive(Default)]
struct RecordingSoe {
    binaries: Vec<Indexed<Binary>>,
    analogs: Vec<Indexed<Analog>>,
    counters: Vec<Indexed<Counter>>,
    unsolicited_fragments: usize,
    solicited_fragments: usize,
}

impl SoeHandler for RecordingSoe {
    fn begin_fragment(&mut self, info: ResponseInfo) {
        if info.unsolicited {
            self.unsolicited_fragments += 1;
        } else {
            self.solicited_fragments += 1;
        }
    }
    fn process_binary(&mut self, _info: HeaderInfo, values: &[Indexed<Binary>]) {
        self.binaries.extend_from_slice(values);
    }
    fn process_analog(&mut self, _info: HeaderInfo, values: &[Indexed<Analog>]) {
        self.analogs.extend_from_slice(values);
    }
    fn process_counter(&mut self, _info: HeaderInfo, values: &[Indexed<Counter>]) {
        self.counters.extend_from_slice(values);
    }
}

#[derive(Default)]
struct AcceptingCommandHandler {
    select_crobs: Vec<(u16, ControlRelayOutputBlock)>,
    operate_crobs: Vec<(u16, ControlRelayOutputBlock, OperateType)>,
    operate_ao_i32s: Vec<(u16, AnalogOutputInt32, OperateType)>,
}

impl CommandHandler for AcceptingCommandHandler {
    fn select_crob(&mut self, command: &ControlRelayOutputBlock, index: u16) -> CommandStatus {
        self.select_crobs.push((index, *command));
        CommandStatus::Success
    }
    fn operate_crob(
        &mut self,
        command: &ControlRelayOutputBlock,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        self.operate_crobs.push((index, *command, op_type));
        CommandStatus::Success
    }
    fn select_ao_i32(&mut self, _command: &AnalogOutputInt32, _index: u16) -> CommandStatus {
        CommandStatus::Success
    }
    fn operate_ao_i32(
        &mut self,
        command: &AnalogOutputInt32,
        index: u16,
        op_type: OperateType,
    ) -> CommandStatus {
        self.operate_ao_i32s.push((index, *command, op_type));
        CommandStatus::Success
    }
}

#[derive(Default)]
struct TestOutstationApp {
    last_written_time: Option<DnpTime>,
    cold_restarts: usize,
}

impl OutstationApplication for TestOutstationApp {
    fn supports_write_absolute_time(&self) -> bool {
        true
    }
    fn write_absolute_time(&mut self, time: DnpTime) -> bool {
        self.last_written_time = Some(time);
        true
    }
    fn cold_restart_support(&self) -> RestartMode {
        RestartMode::SupportedDelayFine
    }
    fn cold_restart(&mut self) -> u16 {
        self.cold_restarts += 1;
        250
    }
}

/// Full 3-layer (Link + Transport + App) in-memory harness connecting a Master
/// and an Outstation deterministically with a [`ManualClock`].
struct FullStackPair {
    clock: ManualClock,
    m_link: LinkLayer,
    m_transport: TransportLayer,
    m_parser: FrameParser,
    master: Master,
    m_app: DefaultMasterApplication,
    soe: RecordingSoe,

    o_link: LinkLayer,
    o_transport: TransportLayer,
    o_parser: FrameParser,
    outstation: Outstation,
    o_cmd: AcceptingCommandHandler,
    o_app: TestOutstationApp,
}

impl FullStackPair {
    fn new(m_params: MasterParams, o_cfg: OutstationConfig) -> Self {
        let mut m_link_cfg = LinkConfig::new(true);
        m_link_cfg.local_addr = 1;
        m_link_cfg.remote_addr = 10;
        let mut o_link_cfg = LinkConfig::new(false);
        o_link_cfg.local_addr = 10;
        o_link_cfg.remote_addr = 1;

        Self {
            clock: ManualClock::new(),
            m_link: LinkLayer::new(m_link_cfg),
            m_transport: TransportLayer::new(2048),
            m_parser: FrameParser::new(),
            master: Master::new(m_params, Addresses::new(1, 10)),
            m_app: DefaultMasterApplication,
            soe: RecordingSoe::default(),

            o_link: LinkLayer::new(o_link_cfg),
            o_transport: TransportLayer::new(2048),
            o_parser: FrameParser::new(),
            outstation: Outstation::new(o_cfg, Addresses::new(10, 1)),
            o_cmd: AcceptingCommandHandler::default(),
            o_app: TestOutstationApp::default(),
        }
    }

    fn bring_online(&mut self) {
        let now = self.clock.now();
        self.m_link.on_lower_layer_up(now);
        self.o_link.on_lower_layer_up(now);
        self.pump_all();
    }

    fn pump_all(&mut self) {
        for _ in 0..64 {
            let mut progressed = false;
            let now = self.clock.now();

            // Master -> Transport -> Link
            if !self.m_transport.is_sending()
                && let Some(apdu) = self.master.pop_transmit()
            {
                assert!(self.m_transport.begin_transmit(apdu, &mut self.m_link, now));
                progressed = true;
            }
            // Outstation -> Transport -> Link
            if !self.o_transport.is_sending()
                && let Some(apdu) = self.outstation.pop_transmit()
            {
                assert!(self.o_transport.begin_transmit(apdu, &mut self.o_link, now));
                progressed = true;
            }

            // Wire: Master Link TX -> Outstation Parser -> Outstation Link RX
            while let Some(frame) = self.m_link.pop_transmit() {
                progressed = true;
                self.m_link.on_tx_ready();
                for rf in self.o_parser.push_bytes(&frame) {
                    self.o_link.on_frame(&rf.fields, &rf.payload, now);
                }
            }
            // Wire: Outstation Link TX -> Master Parser -> Master Link RX
            while let Some(frame) = self.o_link.pop_transmit() {
                progressed = true;
                self.o_link.on_tx_ready();
                for rf in self.m_parser.push_bytes(&frame) {
                    self.m_link.on_frame(&rf.fields, &rf.payload, now);
                }
            }

            // Drain Link events -> Transport -> App (Master)
            for ev in self.m_link.drain_events() {
                progressed = true;
                self.m_transport.handle_link_event(&ev);
            }
            for ev in self.m_transport.drain_events() {
                progressed = true;
                match ev {
                    TransportEvent::LowerLayerUp => {
                        self.master.on_lower_layer_up(now, &mut self.m_app)
                    }
                    TransportEvent::LowerLayerDown => {
                        self.master.on_lower_layer_down(&mut self.m_app)
                    }
                    TransportEvent::TxReady => self.master.on_tx_ready(now, &mut self.m_app),
                    TransportEvent::Message(msg) => {
                        self.master
                            .on_receive(&msg, now, &mut self.soe, &mut self.m_app);
                    }
                }
            }

            // Drain Link events -> Transport -> App (Outstation)
            for ev in self.o_link.drain_events() {
                progressed = true;
                self.o_transport.handle_link_event(&ev);
            }
            for ev in self.o_transport.drain_events() {
                progressed = true;
                match ev {
                    TransportEvent::LowerLayerUp => {
                        self.outstation.on_lower_layer_up(now, &self.o_app)
                    }
                    TransportEvent::LowerLayerDown => self.outstation.on_lower_layer_down(),
                    TransportEvent::TxReady => self.outstation.on_tx_ready(now, &self.o_app),
                    TransportEvent::Message(msg) => {
                        self.outstation
                            .on_receive(&msg, now, &mut self.o_cmd, &mut self.o_app);
                    }
                }
            }

            if !progressed {
                break;
            }
        }
    }
}

#[test]
fn database_event_buffer_overflow_and_deadband() {
    let mut cfg = OutstationConfig {
        database: DatabaseConfig::all_types(3),
        event_buffer: EventBufferConfig::all_types(2),
        ..Default::default()
    };
    cfg.database.analog.get_mut(&0).unwrap().deadband = 5.0;

    let mut o = Outstation::new(cfg, Addresses::new(10, 1));
    // Establish initial online baseline at 0.0 without generating an event
    o.database
        .update_analog(0, Analog::new(0.0), EventMode::Suppress);
    // Analog within deadband -> no event
    o.database
        .update_analog(0, Analog::new(3.0), EventMode::Detect);
    assert_eq!(o.database.events.len(), 0);
    // Analog beyond deadband -> event
    o.database
        .update_analog(0, Analog::new(6.0), EventMode::Detect);
    assert_eq!(o.database.events.len(), 1);

    // Overflow binary event buffer (capacity 2)
    o.database
        .update_binary(0, Binary::new(true), EventMode::Force);
    o.database
        .update_binary(0, Binary::new(false), EventMode::Force);
    assert!(!o.database.events.is_overflown());
    o.database
        .update_binary(0, Binary::new(true), EventMode::Force);
    assert!(o.database.events.is_overflown());
}

#[test]
fn full_stack_startup_integrity_poll_and_clear_restart() {
    let mut o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(3),
        ..Default::default()
    };
    o_cfg.params.allow_unsolicited = true;

    let mut pair = FullStackPair::new(MasterParams::default(), o_cfg);
    pair.outstation
        .database
        .update_binary(0, Binary::new(true), EventMode::Suppress);
    pair.outstation
        .database
        .update_analog(1, Analog::new(42.0), EventMode::Suppress);
    pair.outstation
        .database
        .update_counter(2, Counter::new(999), EventMode::Suppress);

    pair.bring_online();

    // Master should have completed DisableUnsol, ClearRestart, and StartupIntegrityPoll!
    let events = pair.master.drain_events();
    assert!(events.iter().any(|e| matches!(
        e,
        MasterEvent::TaskComplete {
            result: TaskCompletion::Success,
            ..
        }
    )));
    assert!(
        pair.soe
            .binaries
            .iter()
            .any(|b| b.index == 0 && b.value.value)
    );
    assert!(
        pair.soe
            .analogs
            .iter()
            .any(|a| a.index == 1 && (a.value.value - 42.0).abs() < 1e-6)
    );
    assert!(
        pair.soe
            .counters
            .iter()
            .any(|c| c.index == 2 && c.value.value == 999)
    );
}

#[test]
fn full_stack_sbo_and_direct_operate_commands() {
    let o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    let m_params = MasterParams {
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::NONE,
        ..Default::default()
    };

    let mut pair = FullStackPair::new(m_params, o_cfg);
    pair.bring_online();

    // 1) Select-Before-Operate CROB
    let crob = ControlRelayOutputBlock::new(
        OperationType::PulseOn,
        TripCloseCode::Close,
        false,
        1,
        100,
        100,
        CommandStatus::Success,
    );
    let mut cs = CommandSet::new();
    cs.add_crob(1, crob);
    pair.master
        .select_and_operate(cs, pair.clock.now(), &mut pair.m_app);
    pair.pump_all();

    assert_eq!(pair.o_cmd.select_crobs.len(), 1);
    assert_eq!(pair.o_cmd.operate_crobs.len(), 1);
    assert_eq!(
        pair.o_cmd.operate_crobs[0].2,
        OperateType::SelectBeforeOperate
    );

    let events = pair.master.drain_events();
    assert!(events.iter().any(|e| matches!(
        e,
        MasterEvent::CommandComplete(r)
            if r.summary == TaskCompletion::Success
                && r.points.len() == 1
                && r.points[0].status == CommandStatus::Success
    )));

    // 2) Direct Operate AnalogOutputInt32
    let mut cs2 = CommandSet::new();
    cs2.add_ao_i32(0, AnalogOutputInt32::new(777));
    pair.master
        .direct_operate(cs2, false, pair.clock.now(), &mut pair.m_app);
    pair.pump_all();

    assert_eq!(pair.o_cmd.operate_ao_i32s.len(), 1);
    assert_eq!(pair.o_cmd.operate_ao_i32s[0].1.value, 777);
    assert_eq!(pair.o_cmd.operate_ao_i32s[0].2, OperateType::DirectOperate);
}

#[test]
fn full_stack_unsolicited_events_and_confirm() {
    let mut o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    o_cfg.params.allow_unsolicited = true;

    let m_params = MasterParams {
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::ALL_EVENT_CLASSES,
        ignore_restart_iin: true,
        ..Default::default()
    };

    let mut pair = FullStackPair::new(m_params, o_cfg);
    pair.bring_online();

    // Null unsolicited should have been sent and confirmed; now trigger a binary event
    pair.outstation
        .database
        .update_binary(1, Binary::new(true), EventMode::Force);
    pair.outstation
        .check_for_task_start(pair.clock.now(), &pair.o_app);
    pair.pump_all();

    assert!(pair.soe.unsolicited_fragments >= 1);
    assert!(
        pair.soe
            .binaries
            .iter()
            .any(|b| b.index == 1 && b.value.value)
    );
    // Event buffer should be cleared after master's unsolicited CONFIRM
    assert!(pair.outstation.database.events.is_empty());
}

#[test]
fn full_stack_multi_fragment_read_and_restart() {
    let mut o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(20),
        ..Default::default()
    };
    // Tiny fragment size forces multi-fragment response + CON=1 + CONFIRM handshake!
    o_cfg.params.max_tx_frag_size = 32;

    let m_params = MasterParams {
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::NONE,
        ignore_restart_iin: true,
        ..Default::default()
    };

    let mut pair = FullStackPair::new(m_params, o_cfg);
    for i in 0..20 {
        pair.outstation.database.update_analog(
            i,
            Analog::new(i as f64 * 10.0),
            EventMode::Suppress,
        );
    }
    pair.bring_online();

    pair.master.scan_range(
        GroupVariation::Group30Var1,
        Range::new(0, 19),
        pair.clock.now(),
        &mut pair.m_app,
    );
    pair.pump_all();

    assert!(pair.soe.solicited_fragments > 1);
    assert_eq!(pair.soe.analogs.len(), 20);
    assert!((pair.soe.analogs[19].value.value - 190.0).abs() < 1e-6);

    // Cold restart
    pair.master
        .restart(RestartType::Cold, pair.clock.now(), &mut pair.m_app);
    pair.pump_all();
    assert_eq!(pair.o_app.cold_restarts, 1);
    let events = pair.master.drain_events();
    assert!(events.iter().any(|e| matches!(
        e,
        MasterEvent::RestartComplete {
            result: TaskCompletion::Success,
            delay_ms: Some(250),
        }
    )));
}

#[test]
fn master_response_timeout() {
    let m_params = MasterParams {
        response_timeout: Duration::from_secs(2),
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::ALL_CLASSES,
        unsol_class_mask: ClassField::NONE,
        ..Default::default()
    };

    let mut clock = ManualClock::new();
    let mut master = Master::new(m_params, Addresses::new(1, 10));
    let mut app = DefaultMasterApplication;

    master.on_lower_layer_up(clock.now(), &mut app);
    let req = master.pop_transmit().expect("integrity poll");
    assert!(!req.payload.is_empty());
    master.on_tx_ready(clock.now(), &mut app);

    clock.advance(Duration::from_secs(2));
    master.on_timeout(clock.now(), &mut app);

    let events = master.drain_events();
    assert!(events.iter().any(|e| matches!(
        e,
        MasterEvent::TaskComplete {
            result: TaskCompletion::FailureResponseTimeout,
            ..
        }
    )));
}

#[test]
fn outstation_rejects_operate_without_select() {
    let o_cfg = OutstationConfig {
        database: DatabaseConfig::all_types(2),
        ..Default::default()
    };
    let clock = ManualClock::new();
    let mut o = Outstation::new(o_cfg, Addresses::new(10, 1));
    let mut cmd = AcceptingCommandHandler::default();
    let mut app = DefaultOutstationApplication;
    o.on_lower_layer_up(clock.now(), &app);

    // Build an OPERATE request directly without a prior SELECT
    let mut w = opendnp3::app::ApduWriter::new_request(
        64,
        opendnp3::app::AppControlField::request(1),
        opendnp3::enums::FunctionCode::Operate,
    );
    let mut cs = CommandSet::new();
    cs.add_crob(
        0,
        ControlRelayOutputBlock::new(
            OperationType::PulseOn,
            TripCloseCode::Nul,
            false,
            1,
            50,
            50,
            CommandStatus::Success,
        ),
    );
    cs.write(&mut w);

    o.on_receive(
        &Message::new(Addresses::new(1, 10), w.into_bytes()),
        clock.now(),
        &mut cmd,
        &mut app,
    );
    assert!(cmd.operate_crobs.is_empty());
    let resp = o.pop_transmit().expect("response");
    // Response should echo CROB with status == NoSelect (2)
    assert_eq!(
        *resp.payload.last().unwrap(),
        CommandStatus::NoSelect.to_u8()
    );
    // And IIN should still have DeviceRestart set
    let iin = IinField::new(resp.payload[2], resp.payload[3]);
    assert!(iin.is_set(IinBit::DeviceRestart));
}
