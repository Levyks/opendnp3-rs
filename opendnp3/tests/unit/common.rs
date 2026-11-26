#![allow(dead_code)]

use std::time::Duration;

use opendnp3::clock::ManualClock;
use opendnp3::enums::LinkFunction;
use opendnp3::link::constants::{LPDU_DATA_BLOCK_SIZE, LPDU_HEADER_SIZE, LPDU_MAX_FRAME_SIZE};
use opendnp3::link::{
    Addresses, LinkConfig, LinkEvent, LinkHeaderFields, LinkLayer, TransportSegment, crc, frame,
};
use opendnp3::util::WriteCursor;
use opendnp3::util::hex::{from_hex, to_hex};

/// Repair header and body CRCs in a hex-encoded FT3 frame (mirrors C++ `RepairCRC`).
pub fn repair_crc(hex: &str) -> String {
    let mut bytes = from_hex(hex).expect("valid hex");
    assert!(bytes.len() >= LPDU_HEADER_SIZE);
    crc::add_crc(&mut bytes, 8);
    let mut offset = LPDU_HEADER_SIZE;
    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        assert!(remaining >= 2);
        let block_len = (remaining - 2).min(LPDU_DATA_BLOCK_SIZE);
        crc::add_crc(&mut bytes[offset..], block_len);
        offset += block_len + 2;
    }
    to_hex(&bytes)
}

pub fn format_user_data(
    is_master: bool,
    is_confirmed: bool,
    dest: u16,
    src: u16,
    data_hex: &str,
    fcb: bool,
) -> String {
    let data = from_hex(data_hex).expect("valid hex");
    assert!(data.len() <= 250);
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    if is_confirmed {
        frame::format_confirmed_user_data(&mut cursor, is_master, fcb, dest, src, &data).unwrap();
    } else {
        frame::format_unconfirmed_user_data(&mut cursor, is_master, dest, src, &data).unwrap();
    }
    to_hex(cursor.written())
}

pub fn link_hex_ack(is_master: bool, is_rcv_buff_full: bool, dest: u16, src: u16) -> String {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_ack(&mut cursor, is_master, is_rcv_buff_full, dest, src);
    to_hex(cursor.written())
}

pub fn link_hex_link_status(
    is_master: bool,
    is_rcv_buff_full: bool,
    dest: u16,
    src: u16,
) -> String {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_link_status(&mut cursor, is_master, is_rcv_buff_full, dest, src);
    to_hex(cursor.written())
}

pub fn link_hex_request_link_status(is_master: bool, dest: u16, src: u16) -> String {
    let mut buf = [0u8; LPDU_MAX_FRAME_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    frame::format_request_link_status(&mut cursor, is_master, dest, src);
    to_hex(cursor.written())
}

pub fn incrementing_bytes(len: usize, start: u8) -> Vec<u8> {
    (0..len).map(|i| start.wrapping_add(i as u8)).collect()
}

pub struct MockTransportSegment {
    data: Vec<u8>,
    max_segment_size: usize,
    offset: usize,
    addresses: Addresses,
}

impl MockTransportSegment {
    pub fn new(max_segment_size: usize, data: Vec<u8>, addresses: Addresses) -> Self {
        Self {
            data,
            max_segment_size,
            offset: 0,
            addresses,
        }
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

impl TransportSegment for MockTransportSegment {
    fn addresses(&self) -> Addresses {
        self.addresses
    }

    fn has_value(&self) -> bool {
        self.offset < self.data.len()
    }

    fn segment(&mut self) -> &[u8] {
        let end = (self.offset + self.max_segment_size).min(self.data.len());
        &self.data[self.offset..end]
    }

    fn advance(&mut self) -> bool {
        self.offset = (self.offset + self.max_segment_size).min(self.data.len());
        self.has_value()
    }
}

pub struct LinkLayerFixture {
    pub link: LinkLayer,
    pub clock: ManualClock,
    pub upper_online: bool,
    pub received_queue: Vec<String>,
    pub tx_ready_count: usize,
    pub writes: Vec<String>,
    pub num_keep_alive_transmissions: usize,
    pub num_keep_alive_failures: usize,
    pub num_keep_alive_replies: usize,
}

impl LinkLayerFixture {
    pub fn new(config: LinkConfig) -> Self {
        Self {
            link: LinkLayer::new(config),
            clock: ManualClock::new(),
            upper_online: false,
            received_queue: Vec::new(),
            tx_ready_count: 0,
            writes: Vec::new(),
            num_keep_alive_transmissions: 0,
            num_keep_alive_failures: 0,
            num_keep_alive_replies: 0,
        }
    }

    pub fn default_master() -> Self {
        Self::new(LinkConfig::new(true))
    }

    pub fn drain(&mut self) {
        while let Some(frame) = self.link.pop_transmit() {
            self.writes.push(to_hex(&frame));
        }
        for ev in self.link.drain_events() {
            match ev {
                LinkEvent::LowerLayerUp => self.upper_online = true,
                LinkEvent::LowerLayerDown => self.upper_online = false,
                LinkEvent::Message(msg) => self.received_queue.push(to_hex(&msg.payload)),
                LinkEvent::TxReady => self.tx_ready_count += 1,
                LinkEvent::KeepAliveInitiated => self.num_keep_alive_transmissions += 1,
                LinkEvent::KeepAliveFailure => self.num_keep_alive_failures += 1,
                LinkEvent::KeepAliveSuccess => self.num_keep_alive_replies += 1,
                _ => {}
            }
        }
    }

    pub fn on_lower_layer_up(&mut self) -> bool {
        let ok = self.link.on_lower_layer_up(self.clock.now());
        self.drain();
        ok
    }

    pub fn on_lower_layer_down(&mut self) -> bool {
        let ok = self.link.on_lower_layer_down();
        self.drain();
        ok
    }

    pub fn on_tx_ready(&mut self) -> bool {
        let ok = self.link.on_tx_ready();
        self.drain();
        ok
    }

    #[allow(clippy::too_many_arguments)]
    pub fn on_frame(
        &mut self,
        func: LinkFunction,
        is_master: bool,
        fcb: bool,
        fcvdfc: bool,
        dest: u16,
        src: u16,
        body: &[u8],
    ) -> bool {
        let header = LinkHeaderFields {
            func,
            is_from_master: is_master,
            fcb,
            fcvdfc,
            addresses: Addresses::new(src, dest),
        };
        let ok = self.link.on_frame(&header, body, self.clock.now());
        self.drain();
        ok
    }

    pub fn send(&mut self, seg: Box<dyn TransportSegment>) -> bool {
        let ok = self.link.send(seg, self.clock.now());
        self.drain();
        ok
    }

    pub fn advance_time(&mut self, dur: Duration) {
        self.clock.advance(dur);
        self.link.on_timeout(self.clock.now());
        self.drain();
    }

    pub fn advance_to_next_timer(&mut self) -> bool {
        if let Some(deadline) = self.link.next_wakeup() {
            let now = self.clock.now();
            if deadline > now {
                self.clock.advance(deadline - now);
            }
            self.link.on_timeout(self.clock.now());
            self.drain();
            true
        } else {
            false
        }
    }

    pub fn num_total_writes(&self) -> usize {
        self.writes.len()
    }

    pub fn pop_last_write(&mut self) -> String {
        self.writes.last().cloned().unwrap_or_default()
    }
}

// =========================================================================
// Shared Outstation Test Fixture (mirrors C++ OutstationTestObject)
// =========================================================================

use opendnp3::Message;
use opendnp3::app::{
    AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    ControlRelayOutputBlock, DnpTime, Range,
};
use opendnp3::enums::{CommandStatus, OperateType, RestartMode};
use opendnp3::link::LinkBroadcastAddress;
use opendnp3::outstation::{
    CommandHandler, DatabaseConfig, Outstation, OutstationApplication, OutstationConfig,
};

#[derive(Default)]
pub struct MockCommandHandler {
    pub selects: usize,
    pub operates: usize,
    pub crob_select_status: Option<CommandStatus>,
    pub crob_operate_status: Option<CommandStatus>,
    pub ao_i16: Vec<(u16, i16)>,
    pub ao_i32: Vec<(u16, i32)>,
    pub ao_f32: Vec<(u16, f32)>,
    pub ao_f64: Vec<(u16, f64)>,
}

impl CommandHandler for MockCommandHandler {
    fn select_crob(&mut self, _: &ControlRelayOutputBlock, _: u16) -> CommandStatus {
        self.selects += 1;
        self.crob_select_status.unwrap_or(CommandStatus::Success)
    }
    fn operate_crob(
        &mut self,
        _: &ControlRelayOutputBlock,
        _: u16,
        _: OperateType,
    ) -> CommandStatus {
        self.operates += 1;
        self.crob_operate_status.unwrap_or(CommandStatus::Success)
    }
    fn select_ao_i32(&mut self, _: &AnalogOutputInt32, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_i32(
        &mut self,
        c: &AnalogOutputInt32,
        index: u16,
        _: OperateType,
    ) -> CommandStatus {
        self.operates += 1;
        self.ao_i32.push((index, c.value));
        CommandStatus::Success
    }
    fn select_ao_i16(&mut self, _: &AnalogOutputInt16, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_i16(
        &mut self,
        c: &AnalogOutputInt16,
        index: u16,
        _: OperateType,
    ) -> CommandStatus {
        self.operates += 1;
        self.ao_i16.push((index, c.value));
        CommandStatus::Success
    }
    fn select_ao_f32(&mut self, _: &AnalogOutputFloat32, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_f32(
        &mut self,
        c: &AnalogOutputFloat32,
        index: u16,
        _: OperateType,
    ) -> CommandStatus {
        self.operates += 1;
        self.ao_f32.push((index, c.value));
        CommandStatus::Success
    }
    fn select_ao_f64(&mut self, _: &AnalogOutputDouble64, _: u16) -> CommandStatus {
        self.selects += 1;
        CommandStatus::Success
    }
    fn operate_ao_f64(
        &mut self,
        c: &AnalogOutputDouble64,
        index: u16,
        _: OperateType,
    ) -> CommandStatus {
        self.operates += 1;
        self.ao_f64.push((index, c.value));
        CommandStatus::Success
    }
}

#[derive(Default)]
pub struct MockOutstationApp {
    pub app_iin: opendnp3::app::IinField,
    pub supports_write_time: bool,
    pub supports_assign_class: bool,
    pub written_time: Option<DnpTime>,
    pub cold_mode: Option<RestartMode>,
    pub warm_mode: Option<RestartMode>,
    pub cold_delay: u16,
    pub warm_delay: u16,
    pub now_time: DnpTime,
    pub class_assignments: Vec<(u8, u8, Range)>,
}

impl OutstationApplication for MockOutstationApp {
    fn application_iin(&self) -> opendnp3::app::IinField {
        self.app_iin
    }
    fn supports_write_absolute_time(&self) -> bool {
        self.supports_write_time
    }
    fn write_absolute_time(&mut self, time: DnpTime) -> bool {
        self.written_time = Some(time);
        true
    }
    fn supports_assign_class(&self) -> bool {
        self.supports_assign_class
    }
    fn record_class_assignment(
        &mut self,
        group: u8,
        class: Option<opendnp3::enums::EventClass>,
        range: Range,
    ) {
        let c = match class {
            None => 0,
            Some(opendnp3::enums::EventClass::Ec1) => 1,
            Some(opendnp3::enums::EventClass::Ec2) => 2,
            Some(opendnp3::enums::EventClass::Ec3) => 3,
        };
        self.class_assignments.push((group, c, range));
    }
    fn cold_restart_support(&self) -> RestartMode {
        self.cold_mode.unwrap_or(RestartMode::Unsupported)
    }
    fn warm_restart_support(&self) -> RestartMode {
        self.warm_mode.unwrap_or(RestartMode::Unsupported)
    }
    fn cold_restart(&mut self) -> u16 {
        self.cold_delay
    }
    fn warm_restart(&mut self) -> u16 {
        self.warm_delay
    }
    fn now_dnp_time(&self) -> DnpTime {
        self.now_time
    }
}

pub struct OutstationFixture {
    pub clock: ManualClock,
    pub os: Outstation,
    pub cmd: MockCommandHandler,
    pub app: MockOutstationApp,
    pub writes: Vec<String>,
}

impl OutstationFixture {
    pub fn new(mut config: OutstationConfig, db_cfg: DatabaseConfig) -> Self {
        config.database = db_cfg;
        Self {
            clock: ManualClock::new(),
            os: Outstation::new(config, Addresses::new(1024, 1)),
            cmd: MockCommandHandler::default(),
            app: MockOutstationApp::default(),
            writes: Vec::new(),
        }
    }

    pub fn collect(&mut self) {
        while let Some(msg) = self.os.pop_transmit() {
            self.writes.push(to_hex(&msg.payload));
        }
    }

    pub fn lower_layer_up(&mut self) {
        self.os.on_lower_layer_up(self.clock.now(), &self.app);
        self.collect();
    }

    pub fn lower_layer_down(&mut self) {
        self.os.on_lower_layer_down();
        self.collect();
    }

    pub fn on_tx_ready(&mut self) {
        self.os.on_tx_ready(self.clock.now(), &self.app);
        self.collect();
    }

    pub fn send_to_outstation(&mut self, hex: &str) {
        let bytes = from_hex(hex).expect("valid hex");
        self.os.on_receive(
            &Message::new(Addresses::new(1, 1024), bytes),
            self.clock.now(),
            &mut self.cmd,
            &mut self.app,
        );
        self.collect();
    }

    pub fn broadcast_to_outstation(&mut self, addr: LinkBroadcastAddress, hex: &str) {
        let bytes = from_hex(hex).expect("valid hex");
        self.os.on_receive(
            &Message::new(Addresses::new(1, addr as u16), bytes),
            self.clock.now(),
            &mut self.cmd,
            &mut self.app,
        );
        self.collect();
    }

    pub fn transaction(&mut self, f: impl FnOnce(&mut opendnp3::outstation::Database)) {
        f(&mut self.os.database);
        self.os.check_for_task_start(self.clock.now(), &self.app);
        self.collect();
    }

    pub fn advance_time(&mut self, dur: Duration) {
        self.clock.advance(dur);
        self.os.on_timeout(self.clock.now(), &self.app);
        self.collect();
    }

    pub fn advance_to_next_timer(&mut self) -> bool {
        if let Some(deadline) = self.os.next_wakeup() {
            let now = self.clock.now();
            if deadline > now {
                self.clock.advance(deadline - now);
            }
            self.os.on_timeout(self.clock.now(), &self.app);
            self.collect();
            true
        } else {
            false
        }
    }

    pub fn pop_write(&mut self) -> String {
        if self.writes.is_empty() {
            String::new()
        } else {
            self.writes.remove(0)
        }
    }

    pub fn has_no_data(&self) -> bool {
        self.writes.is_empty()
    }
}

// =========================================================================
// Shared Master Test Fixture (mirrors C++ MasterTestFixture)
// =========================================================================

use opendnp3::app::{ClassField, IinField};
use opendnp3::enums::{MasterTaskType, TaskCompletion};
use opendnp3::master::{Master, MasterApplication, MasterEvent, MasterParams, NullSoeHandler};

pub struct MockMasterApp {
    pub now_dnp: DnpTime,
    pub received_iins: Vec<IinField>,
    pub completed: Vec<(MasterTaskType, TaskCompletion)>,
}

impl Default for MockMasterApp {
    fn default() -> Self {
        Self {
            now_dnp: DnpTime::new(0),
            received_iins: Vec::new(),
            completed: Vec::new(),
        }
    }
}

impl MasterApplication for MockMasterApp {
    fn on_receive_iin(&mut self, iin: IinField) {
        self.received_iins.push(iin);
    }
    fn on_task_complete(&mut self, task_type: MasterTaskType, result: TaskCompletion) {
        self.completed.push((task_type, result));
    }
    fn now_dnp_time(&self) -> DnpTime {
        self.now_dnp
    }
}

pub fn no_startup_tasks() -> MasterParams {
    MasterParams {
        disable_unsol_on_startup: false,
        startup_integrity_class_mask: ClassField::NONE,
        unsol_class_mask: ClassField::NONE,
        ..Default::default()
    }
}

pub struct MasterFixture {
    pub clock: ManualClock,
    pub master: Master,
    pub app: MockMasterApp,
    pub soe: NullSoeHandler,
    pub events: Vec<MasterEvent>,
}

impl MasterFixture {
    pub fn new(params: MasterParams) -> Self {
        Self::with_addresses(params, Addresses::new(1, 1024))
    }

    pub fn with_addresses(params: MasterParams, addresses: Addresses) -> Self {
        Self {
            clock: ManualClock::new(),
            master: Master::new(params, addresses),
            app: MockMasterApp::default(),
            soe: NullSoeHandler,
            events: Vec::new(),
        }
    }

    pub fn lower_layer_up(&mut self) {
        self.master
            .on_lower_layer_up(self.clock.now(), &mut self.app);
        self.events.extend(self.master.drain_events());
    }

    pub fn lower_layer_down(&mut self) {
        self.master.on_lower_layer_down(&mut self.app);
        self.events.extend(self.master.drain_events());
    }

    pub fn pop_write(&mut self) -> String {
        if let Some(msg) = self.master.pop_transmit() {
            to_hex(&msg.payload)
        } else {
            String::new()
        }
    }

    pub fn on_tx_ready(&mut self) {
        self.master.on_tx_ready(self.clock.now(), &mut self.app);
        self.events.extend(self.master.drain_events());
    }

    pub fn send_to_master(&mut self, hex: &str) {
        let bytes = from_hex(hex).expect("valid hex");
        self.master.on_receive(
            &Message::new(Addresses::new(1024, 1), bytes),
            self.clock.now(),
            &mut self.soe,
            &mut self.app,
        );
        self.events.extend(self.master.drain_events());
    }

    pub fn advance_time(&mut self, dur: Duration) {
        self.clock.advance(dur);
        self.master.on_timeout(self.clock.now(), &mut self.app);
        self.events.extend(self.master.drain_events());
    }

    pub fn advance_to_next_timer(&mut self) -> bool {
        if let Some(deadline) = self.master.next_wakeup() {
            let now = self.clock.now();
            if deadline > now {
                self.clock.advance(deadline - now);
            }
            self.master.on_timeout(self.clock.now(), &mut self.app);
            self.events.extend(self.master.drain_events());
            true
        } else {
            false
        }
    }
}
