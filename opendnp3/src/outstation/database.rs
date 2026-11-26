//! Outstation static database and change-detection engine.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use super::config::{
    AnalogConfig, AnalogOutputStatusConfig, BinaryConfig, BinaryOutputStatusConfig, CounterConfig,
    DatabaseConfig, DoubleBinaryConfig, EventBufferConfig, FrozenCounterConfig, OctetStringConfig,
};
use super::events::{EventBuffer, EventPayload};
use crate::app::{
    Analog, AnalogOutputStatus, ApduWriter, Binary, BinaryOutputStatus, Counter, DnpTime,
    DoubleBitBinary, FrozenCounter, OctetString, Range, TimeAndInterval,
};
use crate::enums::{
    EventClass, EventMode, GroupVariation, StaticAnalogOutputStatusVariation,
    StaticAnalogVariation, StaticBinaryOutputStatusVariation, StaticBinaryVariation,
    StaticCounterVariation, StaticDoubleBinaryVariation, StaticFrozenCounterVariation,
    StaticTimeAndIntervalVariation,
};

#[derive(Debug, Clone)]
struct StaticCell<V, C> {
    value: V,
    last_event_value: V,
    config: C,
}

impl<V: Clone, C> StaticCell<V, C> {
    fn new(value: V, config: C) -> Self {
        Self {
            last_event_value: value.clone(),
            value,
            config,
        }
    }
}

#[derive(Debug, Clone)]
struct StaticSelection {
    gv: GroupVariation,
    indices: VecDeque<u16>,
}

fn abs_f64(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

/// Holds all static measurement points and the associated [`EventBuffer`].
#[derive(Debug, Clone)]
pub struct Database {
    binary: BTreeMap<u16, StaticCell<Binary, BinaryConfig>>,
    double_binary: BTreeMap<u16, StaticCell<DoubleBitBinary, DoubleBinaryConfig>>,
    binary_output_status: BTreeMap<u16, StaticCell<BinaryOutputStatus, BinaryOutputStatusConfig>>,
    counter: BTreeMap<u16, StaticCell<Counter, CounterConfig>>,
    frozen_counter: BTreeMap<u16, StaticCell<FrozenCounter, FrozenCounterConfig>>,
    analog: BTreeMap<u16, StaticCell<Analog, AnalogConfig>>,
    analog_output_status: BTreeMap<u16, StaticCell<AnalogOutputStatus, AnalogOutputStatusConfig>>,
    octet_string: BTreeMap<u16, StaticCell<OctetString, OctetStringConfig>>,
    time_and_interval: BTreeMap<u16, (TimeAndInterval, StaticTimeAndIntervalVariation)>,

    pub events: EventBuffer,
    static_selections: VecDeque<StaticSelection>,
}

impl Database {
    pub fn new(db_cfg: &DatabaseConfig, event_cfg: EventBufferConfig) -> Self {
        let mut db = Self {
            binary: BTreeMap::new(),
            double_binary: BTreeMap::new(),
            binary_output_status: BTreeMap::new(),
            counter: BTreeMap::new(),
            frozen_counter: BTreeMap::new(),
            analog: BTreeMap::new(),
            analog_output_status: BTreeMap::new(),
            octet_string: BTreeMap::new(),
            time_and_interval: BTreeMap::new(),
            events: EventBuffer::new(event_cfg),
            static_selections: VecDeque::new(),
        };
        let restart = crate::app::Flags::new(0x02);
        for (&idx, &cfg) in &db_cfg.binary {
            db.binary
                .insert(idx, StaticCell::new(Binary::with_flags(restart), cfg));
        }
        for (&idx, &cfg) in &db_cfg.double_binary {
            db.double_binary.insert(
                idx,
                StaticCell::new(DoubleBitBinary::with_flags(restart), cfg),
            );
        }
        for (&idx, &cfg) in &db_cfg.binary_output_status {
            db.binary_output_status.insert(
                idx,
                StaticCell::new(BinaryOutputStatus::with_flags(restart), cfg),
            );
        }
        for (&idx, &cfg) in &db_cfg.counter {
            db.counter
                .insert(idx, StaticCell::new(Counter::with_flags(0, restart), cfg));
        }
        for (&idx, &cfg) in &db_cfg.frozen_counter {
            db.frozen_counter.insert(
                idx,
                StaticCell::new(FrozenCounter::with_flags(0, restart), cfg),
            );
        }
        for (&idx, &cfg) in &db_cfg.analog {
            db.analog
                .insert(idx, StaticCell::new(Analog::with_flags(0.0, restart), cfg));
        }
        for (&idx, &cfg) in &db_cfg.analog_output_status {
            db.analog_output_status.insert(
                idx,
                StaticCell::new(AnalogOutputStatus::with_flags(0.0, restart), cfg),
            );
        }
        for (&idx, &cfg) in &db_cfg.octet_string {
            db.octet_string
                .insert(idx, StaticCell::new(OctetString::default(), cfg));
        }
        for (&idx, &var) in &db_cfg.time_and_interval {
            db.time_and_interval
                .insert(idx, (TimeAndInterval::default(), var));
        }
        db
    }

    // ---- Measurement updates ----

    pub fn update_binary(&mut self, index: u16, value: Binary, mode: EventMode) -> bool {
        let Some(cell) = self.binary.get_mut(&index) else {
            return false;
        };
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => {
                cell.last_event_value.value != value.value
                    || cell.last_event_value.flags != value.flags
            }
        };
        if mode != EventMode::EventOnly {
            cell.value = value;
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value;
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value;
            self.events
                .push(index, class, EventPayload::Binary(value, cell.config.e_var));
        }
        true
    }

    pub fn update_double_bit(
        &mut self,
        index: u16,
        value: DoubleBitBinary,
        mode: EventMode,
    ) -> bool {
        let Some(cell) = self.double_binary.get_mut(&index) else {
            return false;
        };
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => {
                cell.last_event_value.value != value.value
                    || cell.last_event_value.flags != value.flags
            }
        };
        if mode != EventMode::EventOnly {
            cell.value = value;
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value;
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value;
            self.events.push(
                index,
                class,
                EventPayload::DoubleBitBinary(value, cell.config.e_var),
            );
        }
        true
    }

    pub fn update_binary_output_status(
        &mut self,
        index: u16,
        value: BinaryOutputStatus,
        mode: EventMode,
    ) -> bool {
        let Some(cell) = self.binary_output_status.get_mut(&index) else {
            return false;
        };
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => {
                cell.last_event_value.value != value.value
                    || cell.last_event_value.flags != value.flags
            }
        };
        if mode != EventMode::EventOnly {
            cell.value = value;
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value;
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value;
            self.events.push(
                index,
                class,
                EventPayload::BinaryOutputStatus(value, cell.config.e_var),
            );
        }
        true
    }

    pub fn update_counter(&mut self, index: u16, value: Counter, mode: EventMode) -> bool {
        let Some(cell) = self.counter.get_mut(&index) else {
            return false;
        };
        let diff = value.value.abs_diff(cell.last_event_value.value) as f64;
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => {
                cell.last_event_value.flags != value.flags || diff > cell.config.deadband
            }
        };
        if mode != EventMode::EventOnly {
            cell.value = value;
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value;
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value;
            self.events.push(
                index,
                class,
                EventPayload::Counter(value, cell.config.e_var),
            );
        }
        true
    }

    pub fn update_frozen_counter(
        &mut self,
        index: u16,
        value: FrozenCounter,
        mode: EventMode,
    ) -> bool {
        let Some(cell) = self.frozen_counter.get_mut(&index) else {
            return false;
        };
        let diff = value.value.abs_diff(cell.last_event_value.value) as f64;
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => {
                cell.last_event_value.flags != value.flags || diff > cell.config.deadband
            }
        };
        if mode != EventMode::EventOnly {
            cell.value = value;
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value;
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value;
            self.events.push(
                index,
                class,
                EventPayload::FrozenCounter(value, cell.config.e_var),
            );
        }
        true
    }

    pub fn update_analog(&mut self, index: u16, value: Analog, mode: EventMode) -> bool {
        let Some(cell) = self.analog.get_mut(&index) else {
            return false;
        };
        let diff = abs_f64(value.value - cell.last_event_value.value);
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => {
                cell.last_event_value.flags != value.flags || diff > cell.config.deadband
            }
        };
        if mode != EventMode::EventOnly {
            cell.value = value;
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value;
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value;
            self.events
                .push(index, class, EventPayload::Analog(value, cell.config.e_var));
        }
        true
    }

    pub fn update_analog_output_status(
        &mut self,
        index: u16,
        value: AnalogOutputStatus,
        mode: EventMode,
    ) -> bool {
        let Some(cell) = self.analog_output_status.get_mut(&index) else {
            return false;
        };
        let diff = abs_f64(value.value - cell.last_event_value.value);
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => {
                cell.last_event_value.flags != value.flags || diff > cell.config.deadband
            }
        };
        if mode != EventMode::EventOnly {
            cell.value = value;
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value;
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value;
            self.events.push(
                index,
                class,
                EventPayload::AnalogOutputStatus(value, cell.config.e_var),
            );
        }
        true
    }

    pub fn update_octet_string(&mut self, index: u16, value: OctetString, mode: EventMode) -> bool {
        let Some(cell) = self.octet_string.get_mut(&index) else {
            return false;
        };
        let is_event = match mode {
            EventMode::Force | EventMode::EventOnly => true,
            EventMode::Suppress => false,
            EventMode::Detect => cell.last_event_value != value,
        };
        if mode != EventMode::EventOnly {
            cell.value = value.clone();
        }
        if mode == EventMode::Suppress {
            cell.last_event_value = value.clone();
        }
        if is_event && let Some(class) = cell.config.class {
            cell.last_event_value = value.clone();
            self.events
                .push(index, class, EventPayload::OctetString(value));
        }
        true
    }

    pub fn freeze_counters(&mut self, range: Option<Range>, clear: bool, time: DnpTime) {
        let indices: Vec<u16> = self
            .counter
            .keys()
            .copied()
            .filter(|&i| range.is_none_or(|r| i >= r.start && i <= r.stop))
            .collect();
        for idx in indices {
            if let Some(c) = self.counter.get_mut(&idx) {
                let frozen = FrozenCounter::with_flags_and_time(c.value.value, c.value.flags, time);
                if clear {
                    c.value.value = 0;
                }
                self.update_frozen_counter(idx, frozen, EventMode::Detect);
            }
        }
    }

    pub fn assign_class(
        &mut self,
        gv: GroupVariation,
        range: Option<Range>,
        class: Option<EventClass>,
    ) -> usize {
        let in_range = |i: u16| range.is_none_or(|r| i >= r.start && i <= r.stop);
        let mut count = 0usize;
        match gv.group() {
            1 => {
                for (&i, cell) in &mut self.binary {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            3 => {
                for (&i, cell) in &mut self.double_binary {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            10 => {
                for (&i, cell) in &mut self.binary_output_status {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            20 => {
                for (&i, cell) in &mut self.counter {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            21 => {
                for (&i, cell) in &mut self.frozen_counter {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            30 => {
                for (&i, cell) in &mut self.analog {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            40 => {
                for (&i, cell) in &mut self.analog_output_status {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            110 => {
                for (&i, cell) in &mut self.octet_string {
                    if in_range(i) {
                        cell.config.class = class;
                        count += 1;
                    }
                }
            }
            _ => {}
        }
        count
    }

    // ---- Static selection & writing ----

    pub fn clear_static_selections(&mut self) {
        self.static_selections.clear();
    }

    pub fn has_static_selections(&self) -> bool {
        !self.static_selections.is_empty()
    }

    pub fn select_class0(&mut self) {
        self.select_static(GroupVariation::Group1Var0, None);
        self.select_static(GroupVariation::Group3Var0, None);
        self.select_static(GroupVariation::Group10Var0, None);
        self.select_static(GroupVariation::Group20Var0, None);
        self.select_static(GroupVariation::Group21Var0, None);
        self.select_static(GroupVariation::Group30Var0, None);
        self.select_static(GroupVariation::Group40Var0, None);
        self.select_static(GroupVariation::Group110Var0, None);
    }

    pub fn select_static(&mut self, gv: GroupVariation, range: Option<Range>) -> bool {
        let filter = |i: &u16| range.is_none_or(|r| *i >= r.start && *i <= r.stop);
        let (resolved_gv, indices): (GroupVariation, VecDeque<u16>) = match gv.group() {
            1 => {
                let ids: VecDeque<u16> = self.binary.keys().copied().filter(filter).collect();
                let target = if gv.variation() == 0 {
                    ids.front()
                        .and_then(|i| self.binary.get(i))
                        .map(|c| match c.config.s_var {
                            StaticBinaryVariation::Group1Var1 => GroupVariation::Group1Var1,
                            StaticBinaryVariation::Group1Var2 => GroupVariation::Group1Var2,
                        })
                        .unwrap_or(GroupVariation::Group1Var2)
                } else {
                    gv
                };
                (target, ids)
            }
            3 => {
                let ids: VecDeque<u16> =
                    self.double_binary.keys().copied().filter(filter).collect();
                let target = if gv.variation() == 0 {
                    ids.front()
                        .and_then(|i| self.double_binary.get(i))
                        .map(|c| match c.config.s_var {
                            StaticDoubleBinaryVariation::Group3Var1 => GroupVariation::Group3Var1,
                            StaticDoubleBinaryVariation::Group3Var2 => GroupVariation::Group3Var2,
                        })
                        .unwrap_or(GroupVariation::Group3Var2)
                } else {
                    gv
                };
                (target, ids)
            }
            10 => {
                let ids: VecDeque<u16> = self
                    .binary_output_status
                    .keys()
                    .copied()
                    .filter(filter)
                    .collect();
                let target = if gv.variation() == 0 {
                    ids.front()
                        .and_then(|i| self.binary_output_status.get(i))
                        .map(|c| match c.config.s_var {
                            StaticBinaryOutputStatusVariation::Group10Var2 => {
                                GroupVariation::Group10Var2
                            }
                        })
                        .unwrap_or(GroupVariation::Group10Var2)
                } else {
                    gv
                };
                (target, ids)
            }
            20 => {
                let ids: VecDeque<u16> = self.counter.keys().copied().filter(filter).collect();
                let target = if gv.variation() == 0 {
                    ids.front()
                        .and_then(|i| self.counter.get(i))
                        .map(|c| match c.config.s_var {
                            StaticCounterVariation::Group20Var1 => GroupVariation::Group20Var1,
                            StaticCounterVariation::Group20Var2 => GroupVariation::Group20Var2,
                            StaticCounterVariation::Group20Var5 => GroupVariation::Group20Var5,
                            StaticCounterVariation::Group20Var6 => GroupVariation::Group20Var6,
                        })
                        .unwrap_or(GroupVariation::Group20Var1)
                } else {
                    gv
                };
                (target, ids)
            }
            21 => {
                let ids: VecDeque<u16> =
                    self.frozen_counter.keys().copied().filter(filter).collect();
                let target = if gv.variation() == 0 {
                    ids.front()
                        .and_then(|i| self.frozen_counter.get(i))
                        .map(|c| match c.config.s_var {
                            StaticFrozenCounterVariation::Group21Var1 => {
                                GroupVariation::Group21Var1
                            }
                            StaticFrozenCounterVariation::Group21Var2 => {
                                GroupVariation::Group21Var2
                            }
                            StaticFrozenCounterVariation::Group21Var5 => {
                                GroupVariation::Group21Var5
                            }
                            StaticFrozenCounterVariation::Group21Var6 => {
                                GroupVariation::Group21Var6
                            }
                            StaticFrozenCounterVariation::Group21Var9 => {
                                GroupVariation::Group21Var9
                            }
                            StaticFrozenCounterVariation::Group21Var10 => {
                                GroupVariation::Group21Var10
                            }
                        })
                        .unwrap_or(GroupVariation::Group21Var1)
                } else {
                    gv
                };
                (target, ids)
            }
            30 => {
                let ids: VecDeque<u16> = self.analog.keys().copied().filter(filter).collect();
                let target = if gv.variation() == 0 {
                    ids.front()
                        .and_then(|i| self.analog.get(i))
                        .map(|c| match c.config.s_var {
                            StaticAnalogVariation::Group30Var1 => GroupVariation::Group30Var1,
                            StaticAnalogVariation::Group30Var2 => GroupVariation::Group30Var2,
                            StaticAnalogVariation::Group30Var3 => GroupVariation::Group30Var3,
                            StaticAnalogVariation::Group30Var4 => GroupVariation::Group30Var4,
                            StaticAnalogVariation::Group30Var5 => GroupVariation::Group30Var5,
                            StaticAnalogVariation::Group30Var6 => GroupVariation::Group30Var6,
                        })
                        .unwrap_or(GroupVariation::Group30Var1)
                } else {
                    gv
                };
                (target, ids)
            }
            40 => {
                let ids: VecDeque<u16> = self
                    .analog_output_status
                    .keys()
                    .copied()
                    .filter(filter)
                    .collect();
                let target = if gv.variation() == 0 {
                    ids.front()
                        .and_then(|i| self.analog_output_status.get(i))
                        .map(|c| match c.config.s_var {
                            StaticAnalogOutputStatusVariation::Group40Var1 => {
                                GroupVariation::Group40Var1
                            }
                            StaticAnalogOutputStatusVariation::Group40Var2 => {
                                GroupVariation::Group40Var2
                            }
                            StaticAnalogOutputStatusVariation::Group40Var3 => {
                                GroupVariation::Group40Var3
                            }
                            StaticAnalogOutputStatusVariation::Group40Var4 => {
                                GroupVariation::Group40Var4
                            }
                        })
                        .unwrap_or(GroupVariation::Group40Var1)
                } else {
                    gv
                };
                (target, ids)
            }
            110 => {
                let ids: VecDeque<u16> = self
                    .octet_string
                    .iter()
                    .filter(|(i, c)| filter(i) && !c.value.bytes.is_empty())
                    .map(|(&i, _)| i)
                    .collect();
                (GroupVariation::Group110Var0, ids)
            }
            _ => return false,
        };

        let exact_match = match range {
            Some(r) => indices.len() == (usize::from(r.stop) - usize::from(r.start) + 1),
            None => true,
        };
        if !indices.is_empty() {
            self.static_selections.push_back(StaticSelection {
                gv: resolved_gv,
                indices,
            });
        }
        exact_match
    }

    /// Write selected static points into `writer`. Returns `true` if all
    /// selected points fit, `false` if more fragments are needed.
    pub fn write_static(&mut self, writer: &mut ApduWriter) -> bool {
        while let Some(mut sel) = self.static_selections.pop_front() {
            if sel.gv == GroupVariation::Group110Var0 {
                while let Some(idx) = sel.indices.front().copied() {
                    if let Some(cell) = self.octet_string.get(&idx)
                        && !writer.write_octet_string_prefixed(110, idx, &cell.value)
                    {
                        self.static_selections.push_front(sel);
                        return false;
                    }
                    sel.indices.pop_front();
                }
                continue;
            }

            if sel.gv == GroupVariation::Group1Var1 {
                // Packed single-bit binaries over contiguous runs
                while let Some(start) = sel.indices.front().copied() {
                    let mut run = Vec::new();
                    let mut expected = start;
                    for &idx in &sel.indices {
                        if idx != expected {
                            break;
                        }
                        run.push(self.binary[&idx].value.value);
                        expected = expected.saturating_add(1);
                    }
                    if !writer.write_packed_binaries(start, &run) {
                        self.static_selections.push_front(sel);
                        return false;
                    }
                    for _ in 0..run.len() {
                        sel.indices.pop_front();
                    }
                }
                continue;
            }

            // Standard range-encoded fixed-size static objects (contiguous runs)
            while let Some(start) = sel.indices.front().copied() {
                let hdr_mark = writer.mark();
                let use_u8 = start <= 255 && sel.indices.back().copied().unwrap_or(start) <= 255;
                let stop_offset = if use_u8 {
                    if !writer.write_range_header_u8(sel.gv, start as u8, start as u8) {
                        self.static_selections.push_front(sel);
                        return false;
                    }
                    writer.mark() - 1
                } else {
                    if !writer.write_range_header_u16(sel.gv, start, start) {
                        self.static_selections.push_front(sel);
                        return false;
                    }
                    writer.mark() - 2
                };

                let mut written_count: u16 = 0;
                let mut expected = start;
                let mut last_idx = start;
                let mut full = false;

                while let Some(&idx) = sel.indices.front() {
                    if idx != expected {
                        break;
                    }
                    let item_mark = writer.mark();
                    if !self.write_static_value(writer, sel.gv, idx) {
                        writer.rollback(item_mark);
                        full = true;
                        break;
                    }
                    sel.indices.pop_front();
                    last_idx = idx;
                    written_count += 1;
                    expected = expected.saturating_add(1);
                }

                if written_count == 0 {
                    writer.rollback(hdr_mark);
                    self.static_selections.push_front(sel);
                    return false;
                }

                if use_u8 {
                    writer.patch_u8(stop_offset, last_idx as u8);
                } else {
                    writer.patch_u16(stop_offset, last_idx);
                }

                if full {
                    self.static_selections.push_front(sel);
                    return false;
                }
            }
        }
        true
    }

    fn write_static_value(&self, writer: &mut ApduWriter, gv: GroupVariation, index: u16) -> bool {
        match gv.group() {
            1 => self
                .binary
                .get(&index)
                .is_some_and(|c| writer.write_binary(gv, &c.value)),
            3 => self
                .double_binary
                .get(&index)
                .is_some_and(|c| writer.write_double_bit(gv, &c.value)),
            10 => self
                .binary_output_status
                .get(&index)
                .is_some_and(|c| writer.write_binary_output_status(gv, &c.value)),
            20 => self
                .counter
                .get(&index)
                .is_some_and(|c| writer.write_counter(gv, &c.value)),
            21 => self
                .frozen_counter
                .get(&index)
                .is_some_and(|c| writer.write_frozen_counter(gv, &c.value)),
            30 => self
                .analog
                .get(&index)
                .is_some_and(|c| writer.write_analog(gv, &c.value)),
            40 => self
                .analog_output_status
                .get(&index)
                .is_some_and(|c| writer.write_analog_output_status(gv, &c.value)),
            _ => false,
        }
    }
}
