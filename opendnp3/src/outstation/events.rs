//! Chronological outstation event buffer with per-type limits, overflow
//! tracking, class/type selection, and confirm-based clearing.

use alloc::collections::VecDeque;

use super::config::EventBufferConfig;
use crate::app::{
    Analog, AnalogOutputStatus, ApduWriter, Binary, BinaryOutputStatus, ClassField, Counter,
    DoubleBitBinary, FrozenCounter, OctetString,
};
use crate::enums::{
    EventAnalogOutputStatusVariation, EventAnalogVariation, EventBinaryOutputStatusVariation,
    EventBinaryVariation, EventClass, EventCounterVariation, EventDoubleBinaryVariation,
    EventFrozenCounterVariation, GroupVariation, QualifierCode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Binary,
    DoubleBitBinary,
    BinaryOutputStatus,
    Counter,
    FrozenCounter,
    Analog,
    AnalogOutputStatus,
    OctetString,
}

#[derive(Debug, Clone)]
pub enum EventPayload {
    Binary(Binary, EventBinaryVariation),
    DoubleBitBinary(DoubleBitBinary, EventDoubleBinaryVariation),
    BinaryOutputStatus(BinaryOutputStatus, EventBinaryOutputStatusVariation),
    Counter(Counter, EventCounterVariation),
    FrozenCounter(FrozenCounter, EventFrozenCounterVariation),
    Analog(Analog, EventAnalogVariation),
    AnalogOutputStatus(AnalogOutputStatus, EventAnalogOutputStatusVariation),
    OctetString(OctetString),
}

impl EventPayload {
    pub fn event_type(&self) -> EventType {
        match self {
            Self::Binary(..) => EventType::Binary,
            Self::DoubleBitBinary(..) => EventType::DoubleBitBinary,
            Self::BinaryOutputStatus(..) => EventType::BinaryOutputStatus,
            Self::Counter(..) => EventType::Counter,
            Self::FrozenCounter(..) => EventType::FrozenCounter,
            Self::Analog(..) => EventType::Analog,
            Self::AnalogOutputStatus(..) => EventType::AnalogOutputStatus,
            Self::OctetString(..) => EventType::OctetString,
        }
    }

    pub fn default_gv(&self) -> GroupVariation {
        match self {
            Self::Binary(_, v) => match v {
                EventBinaryVariation::Group2Var1 => GroupVariation::Group2Var1,
                EventBinaryVariation::Group2Var2 => GroupVariation::Group2Var2,
                EventBinaryVariation::Group2Var3 => GroupVariation::Group2Var3,
            },
            Self::DoubleBitBinary(_, v) => match v {
                EventDoubleBinaryVariation::Group4Var1 => GroupVariation::Group4Var1,
                EventDoubleBinaryVariation::Group4Var2 => GroupVariation::Group4Var2,
                EventDoubleBinaryVariation::Group4Var3 => GroupVariation::Group4Var3,
            },
            Self::BinaryOutputStatus(_, v) => match v {
                EventBinaryOutputStatusVariation::Group11Var1 => GroupVariation::Group11Var1,
                EventBinaryOutputStatusVariation::Group11Var2 => GroupVariation::Group11Var2,
            },
            Self::Counter(_, v) => match v {
                EventCounterVariation::Group22Var1 => GroupVariation::Group22Var1,
                EventCounterVariation::Group22Var2 => GroupVariation::Group22Var2,
                EventCounterVariation::Group22Var5 => GroupVariation::Group22Var5,
                EventCounterVariation::Group22Var6 => GroupVariation::Group22Var6,
            },
            Self::FrozenCounter(_, v) => match v {
                EventFrozenCounterVariation::Group23Var1 => GroupVariation::Group23Var1,
                EventFrozenCounterVariation::Group23Var2 => GroupVariation::Group23Var2,
                EventFrozenCounterVariation::Group23Var5 => GroupVariation::Group23Var5,
                EventFrozenCounterVariation::Group23Var6 => GroupVariation::Group23Var6,
            },
            Self::Analog(_, v) => match v {
                EventAnalogVariation::Group32Var1 => GroupVariation::Group32Var1,
                EventAnalogVariation::Group32Var2 => GroupVariation::Group32Var2,
                EventAnalogVariation::Group32Var3 => GroupVariation::Group32Var3,
                EventAnalogVariation::Group32Var4 => GroupVariation::Group32Var4,
                EventAnalogVariation::Group32Var5 => GroupVariation::Group32Var5,
                EventAnalogVariation::Group32Var6 => GroupVariation::Group32Var6,
                EventAnalogVariation::Group32Var7 => GroupVariation::Group32Var7,
                EventAnalogVariation::Group32Var8 => GroupVariation::Group32Var8,
            },
            Self::AnalogOutputStatus(_, v) => match v {
                EventAnalogOutputStatusVariation::Group42Var1 => GroupVariation::Group42Var1,
                EventAnalogOutputStatusVariation::Group42Var2 => GroupVariation::Group42Var2,
                EventAnalogOutputStatusVariation::Group42Var3 => GroupVariation::Group42Var3,
                EventAnalogOutputStatusVariation::Group42Var4 => GroupVariation::Group42Var4,
                EventAnalogOutputStatusVariation::Group42Var5 => GroupVariation::Group42Var5,
                EventAnalogOutputStatusVariation::Group42Var6 => GroupVariation::Group42Var6,
                EventAnalogOutputStatusVariation::Group42Var7 => GroupVariation::Group42Var7,
                EventAnalogOutputStatusVariation::Group42Var8 => GroupVariation::Group42Var8,
            },
            Self::OctetString(_) => GroupVariation::Group111Var0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventState {
    Unselected,
    Selected(GroupVariation),
    Written,
}

#[derive(Debug, Clone)]
struct EventRecord {
    index: u16,
    class: EventClass,
    payload: EventPayload,
    state: EventState,
}

/// Chronological event storage with per-type caps.
#[derive(Debug, Clone)]
pub struct EventBuffer {
    config: EventBufferConfig,
    events: VecDeque<EventRecord>,
    overflow: bool,
}

impl EventBuffer {
    pub fn new(config: EventBufferConfig) -> Self {
        Self {
            config,
            events: VecDeque::new(),
            overflow: false,
        }
    }

    pub fn is_overflown(&self) -> bool {
        self.overflow
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    fn max_for_type(&self, ty: EventType) -> usize {
        match ty {
            EventType::Binary => self.config.max_binary_events,
            EventType::DoubleBitBinary => self.config.max_double_binary_events,
            EventType::BinaryOutputStatus => self.config.max_binary_output_status_events,
            EventType::Counter => self.config.max_counter_events,
            EventType::FrozenCounter => self.config.max_frozen_counter_events,
            EventType::Analog => self.config.max_analog_events,
            EventType::AnalogOutputStatus => self.config.max_analog_output_status_events,
            EventType::OctetString => self.config.max_octet_string_events,
        }
    }

    fn count_for_type(&self, ty: EventType) -> usize {
        self.events
            .iter()
            .filter(|e| e.payload.event_type() == ty)
            .count()
    }

    /// Push a new event; if the per-type buffer is full, drop the oldest event
    /// of that type and set the overflow flag.
    pub fn push(&mut self, index: u16, class: EventClass, payload: EventPayload) {
        let ty = payload.event_type();
        let max = self.max_for_type(ty);
        if max == 0 {
            return;
        }
        if self.count_for_type(ty) >= max {
            if let Some(pos) = self
                .events
                .iter()
                .position(|e| e.payload.event_type() == ty)
            {
                self.events.remove(pos);
            }
            self.overflow = true;
        }
        self.events.push_back(EventRecord {
            index,
            class,
            payload,
            state: EventState::Unselected,
        });
    }

    /// Classes that have at least one non-written event.
    pub fn unwritten_classes(&self) -> ClassField {
        let mut cf = ClassField::NONE;
        for ev in &self.events {
            if ev.state != EventState::Written {
                cf.set(ClassField::from_event_class(ev.class));
            }
        }
        cf
    }

    pub fn has_any_selected(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e.state, EventState::Selected(_)))
    }

    pub fn num_selected(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e.state, EventState::Selected(_)))
            .count()
    }

    pub fn num_unwritten(&self, class: EventClass) -> usize {
        self.events
            .iter()
            .filter(|e| e.class == class && e.state != EventState::Written)
            .count()
    }

    pub fn is_any_type_full(&self) -> bool {
        let types = [
            EventType::Binary,
            EventType::DoubleBitBinary,
            EventType::BinaryOutputStatus,
            EventType::Counter,
            EventType::FrozenCounter,
            EventType::Analog,
            EventType::AnalogOutputStatus,
            EventType::OctetString,
        ];
        types.iter().any(|&ty| {
            let max = self.max_for_type(ty);
            max > 0 && self.count_for_type(ty) >= max
        })
    }

    pub fn has_any_written(&self) -> bool {
        self.events.iter().any(|e| e.state == EventState::Written)
    }

    pub fn unselect_all(&mut self) {
        for ev in &mut self.events {
            ev.state = EventState::Unselected;
        }
    }

    /// Clear all events marked as written (called when master CONFIRMs).
    pub fn clear_written(&mut self) -> usize {
        let before = self.events.len();
        self.events.retain(|e| e.state != EventState::Written);
        let removed = before - self.events.len();
        if removed > 0 {
            self.overflow = false;
        }
        removed
    }

    /// Select up to `max_count` unselected events matching `class`.
    pub fn select_by_class(&mut self, class: EventClass, max_count: usize) -> usize {
        let mut selected = 0;
        for ev in &mut self.events {
            if selected >= max_count {
                break;
            }
            if ev.state == EventState::Unselected && ev.class == class {
                ev.state = EventState::Selected(ev.payload.default_gv());
                selected += 1;
            }
        }
        selected
    }

    /// Select up to `max_count` unselected events matching `event_type`, using
    /// `requested_gv` (or default if variation == 0).
    pub fn select_by_type(
        &mut self,
        ty: EventType,
        requested_gv: GroupVariation,
        max_count: usize,
    ) -> usize {
        let mut selected = 0;
        for ev in &mut self.events {
            if selected >= max_count {
                break;
            }
            if ev.state == EventState::Unselected && ev.payload.event_type() == ty {
                let gv = if requested_gv.variation() == 0 {
                    ev.payload.default_gv()
                } else {
                    requested_gv
                };
                ev.state = EventState::Selected(gv);
                selected += 1;
            }
        }
        selected
    }

    /// Write selected events into `writer`. Returns `true` if ALL selected
    /// events fit; `false` if more fragments are required.
    pub fn write_selected(&mut self, writer: &mut ApduWriter) -> bool {
        let mut i = 0;
        while i < self.events.len() {
            let EventState::Selected(gv) = self.events[i].state else {
                i += 1;
                continue;
            };

            // Octet string events: batch consecutive selected octet strings of the SAME byte length.
            if let EventPayload::OctetString(ref first_oct) = self.events[i].payload {
                let target_len = first_oct.bytes.len();
                if target_len == 0 || target_len > u8::MAX as usize {
                    self.events[i].state = EventState::Written;
                    i += 1;
                    continue;
                }
                let header_mark = writer.mark();
                if !writer.write_bytes(&[
                    111,
                    target_len as u8,
                    QualifierCode::Uint16CntUint16Index.to_u8(),
                ]) {
                    return false;
                }
                let count_offset = writer.mark();
                if !writer.write_u16(0) {
                    writer.rollback(header_mark);
                    return false;
                }
                let mut count: u16 = 0;
                let mut full = false;
                while i < self.events.len() {
                    if !matches!(self.events[i].state, EventState::Selected(_)) {
                        i += 1;
                        continue;
                    }
                    let EventPayload::OctetString(ref oct) = self.events[i].payload else {
                        break;
                    };
                    if oct.bytes.len() != target_len {
                        break;
                    }
                    let item_mark = writer.mark();
                    if !writer.write_u16(self.events[i].index) || !writer.write_bytes(&oct.bytes) {
                        writer.rollback(item_mark);
                        full = true;
                        break;
                    }
                    self.events[i].state = EventState::Written;
                    count += 1;
                    i += 1;
                }
                if count == 0 {
                    writer.rollback(header_mark);
                    return false;
                }
                writer.patch_u16(count_offset, count);
                if full {
                    return false;
                }
                continue;
            }

            // CTO-based relative-time events (Group2Var3, Group4Var3)
            if matches!(gv, GroupVariation::Group2Var3 | GroupVariation::Group4Var3) {
                let first_time = match &self.events[i].payload {
                    EventPayload::Binary(b, _) => b.time,
                    EventPayload::DoubleBitBinary(d, _) => d.time,
                    _ => unreachable!(),
                };
                let is_sync = first_time.quality == crate::enums::TimestampQuality::Synchronized;
                let cto_var = if is_sync { 1u8 } else { 2u8 };
                let cto_base = first_time.value;

                let header_mark = writer.mark();
                // Group51Var1/2: 33 <var> 07 01 <6-byte time>
                if !writer.write_bytes(&[51, cto_var, QualifierCode::Uint8Cnt.to_u8(), 1])
                    || !writer.write_bytes(&first_time.to_le_6bytes())
                    || !writer.write_bytes(&[
                        gv.group(),
                        gv.variation(),
                        QualifierCode::Uint16CntUint16Index.to_u8(),
                    ])
                {
                    writer.rollback(header_mark);
                    return false;
                }
                let count_offset = writer.mark();
                if !writer.write_u16(0) {
                    writer.rollback(header_mark);
                    return false;
                }

                let mut count: u16 = 0;
                let mut full = false;
                while i < self.events.len() {
                    if !matches!(self.events[i].state, EventState::Selected(_)) {
                        i += 1;
                        continue;
                    }
                    if self.events[i].state != EventState::Selected(gv) {
                        break;
                    }
                    let (flags_byte, ev_time) = match &self.events[i].payload {
                        EventPayload::Binary(b, _) => (b.wire_flags(), b.time),
                        EventPayload::DoubleBitBinary(d, _) => (d.wire_flags(), d.time),
                        _ => break,
                    };
                    let ev_sync = ev_time.quality == crate::enums::TimestampQuality::Synchronized;
                    if ev_sync != is_sync || ev_time.value < cto_base {
                        break;
                    }
                    let diff = ev_time.value - cto_base;
                    if diff > u16::MAX as u64 {
                        break;
                    }
                    let item_mark = writer.mark();
                    let ok = writer.write_u16(self.events[i].index)
                        && writer.write_u8(flags_byte)
                        && writer.write_u16(diff as u16);
                    if !ok {
                        writer.rollback(item_mark);
                        full = true;
                        break;
                    }
                    self.events[i].state = EventState::Written;
                    count += 1;
                    i += 1;
                }
                if count == 0 {
                    writer.rollback(header_mark);
                    return false;
                }
                writer.patch_u16(count_offset, count);
                if full {
                    return false;
                }
                continue;
            }

            // Group consecutive selected events with the same GroupVariation into one header.
            let header_mark = writer.mark();
            // Header: group(1) + var(1) + qual(0x28: u16 count + u16 index) + count(2) = 5 bytes
            if !writer.write_bytes(&[
                gv.group(),
                gv.variation(),
                QualifierCode::Uint16CntUint16Index.to_u8(),
            ]) {
                return false;
            }
            let count_offset = writer.mark();
            if !writer.write_u16(0) {
                writer.rollback(header_mark);
                return false;
            }

            let mut count: u16 = 0;
            let mut full = false;
            while i < self.events.len() {
                if !matches!(self.events[i].state, EventState::Selected(_)) {
                    i += 1;
                    continue;
                }
                if self.events[i].state != EventState::Selected(gv) {
                    break;
                }
                let item_mark = writer.mark();
                let ok = writer.write_u16(self.events[i].index)
                    && write_event_value(writer, gv, &self.events[i].payload);
                if !ok {
                    writer.rollback(item_mark);
                    full = true;
                    break;
                }
                self.events[i].state = EventState::Written;
                count += 1;
                i += 1;
            }

            if count == 0 {
                writer.rollback(header_mark);
                return false;
            }
            writer.patch_u16(count_offset, count);
            if full {
                return false;
            }
        }
        true
    }
}

fn write_event_value(writer: &mut ApduWriter, gv: GroupVariation, payload: &EventPayload) -> bool {
    match payload {
        EventPayload::Binary(v, _) => writer.write_binary(gv, v),
        EventPayload::DoubleBitBinary(v, _) => writer.write_double_bit(gv, v),
        EventPayload::BinaryOutputStatus(v, _) => writer.write_binary_output_status(gv, v),
        EventPayload::Counter(v, _) => writer.write_counter(gv, v),
        EventPayload::FrozenCounter(v, _) => writer.write_frozen_counter(gv, v),
        EventPayload::Analog(v, _) => writer.write_analog(gv, v),
        EventPayload::AnalogOutputStatus(v, _) => writer.write_analog_output_status(gv, v),
        EventPayload::OctetString(_) => false,
    }
}
