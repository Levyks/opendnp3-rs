//! DNP3 measurement types, flags, timestamps, and control blocks.

use alloc::vec::Vec;

use crate::enums::{
    AnalogOutputStatusQuality, AnalogQuality, BinaryOutputStatusQuality, BinaryQuality,
    CommandStatus, CounterQuality, DoubleBit, DoubleBitBinaryQuality, FrozenCounterQuality,
    OperationType, TimestampQuality, TripCloseCode,
};

/// 1-byte measurement quality flags wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags {
    pub value: u8,
}

impl Flags {
    pub const fn new(value: u8) -> Self {
        Self { value }
    }

    pub const fn online() -> Self {
        Self { value: 0x01 }
    }

    pub const fn is_set(self, mask: u8) -> bool {
        (self.value & mask) != 0
    }

    pub fn set(&mut self, mask: u8) {
        self.value |= mask;
    }

    pub fn clear(&mut self, mask: u8) {
        self.value &= !mask;
    }

    pub const fn is_set_binary(self, flag: BinaryQuality) -> bool {
        self.is_set(flag as u8)
    }
    pub const fn is_set_double_binary(self, flag: DoubleBitBinaryQuality) -> bool {
        self.is_set(flag as u8)
    }
    pub const fn is_set_binary_output(self, flag: BinaryOutputStatusQuality) -> bool {
        self.is_set(flag as u8)
    }
    pub const fn is_set_counter(self, flag: CounterQuality) -> bool {
        self.is_set(flag as u8)
    }
    pub const fn is_set_frozen_counter(self, flag: FrozenCounterQuality) -> bool {
        self.is_set(flag as u8)
    }
    pub const fn is_set_analog(self, flag: AnalogQuality) -> bool {
        self.is_set(flag as u8)
    }
    pub const fn is_set_analog_output(self, flag: AnalogOutputStatusQuality) -> bool {
        self.is_set(flag as u8)
    }
}

/// 48-bit DNP3 timestamp (milliseconds since Unix epoch) + quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DnpTime {
    pub value: u64,
    pub quality: TimestampQuality,
}

impl DnpTime {
    pub const MAX: u64 = 0x0000_FFFF_FFFF_FFFF;

    pub const fn new(value: u64) -> Self {
        Self {
            value: value & Self::MAX,
            quality: TimestampQuality::Synchronized,
        }
    }

    pub const fn with_quality(value: u64, quality: TimestampQuality) -> Self {
        Self {
            value: value & Self::MAX,
            quality,
        }
    }

    pub const fn invalid() -> Self {
        Self {
            value: 0,
            quality: TimestampQuality::Invalid,
        }
    }

    pub fn from_le_6bytes(bytes: [u8; 6]) -> Self {
        let mut full = [0u8; 8];
        full[..6].copy_from_slice(&bytes);
        Self::new(u64::from_le_bytes(full))
    }

    pub fn to_le_6bytes(self) -> [u8; 6] {
        let full = (self.value & Self::MAX).to_le_bytes();
        [full[0], full[1], full[2], full[3], full[4], full[5]]
    }
}

/// A measurement value paired with its point index.
#[derive(Debug, Clone, PartialEq)]
pub struct Indexed<T> {
    pub index: u16,
    pub value: T,
}

impl<T> Indexed<T> {
    pub const fn new(index: u16, value: T) -> Self {
        Self { index, value }
    }
}

/// Binary input measurement (Group 1 / Group 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Binary {
    pub value: bool,
    pub flags: Flags,
    pub time: DnpTime,
}

impl Binary {
    pub const fn new(value: bool) -> Self {
        let flags = if value { 0x81 } else { 0x01 };
        Self {
            value,
            flags: Flags::new(flags),
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags(flags: Flags) -> Self {
        Self {
            value: (flags.value & 0x80) != 0,
            flags,
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags_and_time(flags: Flags, time: DnpTime) -> Self {
        Self {
            value: (flags.value & 0x80) != 0,
            flags,
            time,
        }
    }

    pub const fn wire_flags(self) -> u8 {
        if self.value {
            self.flags.value | 0x80
        } else {
            self.flags.value & !0x80
        }
    }
}

/// Double-bit binary input measurement (Group 3 / Group 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DoubleBitBinary {
    pub value: DoubleBit,
    pub flags: Flags,
    pub time: DnpTime,
}

impl DoubleBitBinary {
    pub const fn new(value: DoubleBit) -> Self {
        let flags = 0x01 | ((value as u8) << 6);
        Self {
            value,
            flags: Flags::new(flags),
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags(flags: Flags) -> Self {
        Self {
            value: DoubleBit::from_u8(flags.value >> 6),
            flags,
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags_and_time(flags: Flags, time: DnpTime) -> Self {
        Self {
            value: DoubleBit::from_u8(flags.value >> 6),
            flags,
            time,
        }
    }

    pub const fn wire_flags(self) -> u8 {
        (self.flags.value & 0x3F) | ((self.value as u8) << 6)
    }
}

/// Binary output status measurement (Group 10 / Group 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BinaryOutputStatus {
    pub value: bool,
    pub flags: Flags,
    pub time: DnpTime,
}

impl BinaryOutputStatus {
    pub const fn new(value: bool) -> Self {
        let flags = if value { 0x81 } else { 0x01 };
        Self {
            value,
            flags: Flags::new(flags),
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags(flags: Flags) -> Self {
        Self {
            value: (flags.value & 0x80) != 0,
            flags,
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags_and_time(flags: Flags, time: DnpTime) -> Self {
        Self {
            value: (flags.value & 0x80) != 0,
            flags,
            time,
        }
    }

    pub const fn wire_flags(self) -> u8 {
        if self.value {
            self.flags.value | 0x80
        } else {
            self.flags.value & !0x80
        }
    }
}

/// Counter measurement (Group 20 / Group 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counter {
    pub value: u32,
    pub flags: Flags,
    pub time: DnpTime,
}

impl Counter {
    pub const fn new(value: u32) -> Self {
        Self {
            value,
            flags: Flags::online(),
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags(value: u32, flags: Flags) -> Self {
        Self {
            value,
            flags,
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags_and_time(value: u32, flags: Flags, time: DnpTime) -> Self {
        Self { value, flags, time }
    }
}

/// Frozen counter measurement (Group 21 / Group 23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrozenCounter {
    pub value: u32,
    pub flags: Flags,
    pub time: DnpTime,
}

impl FrozenCounter {
    pub const fn new(value: u32) -> Self {
        Self {
            value,
            flags: Flags::online(),
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags(value: u32, flags: Flags) -> Self {
        Self {
            value,
            flags,
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags_and_time(value: u32, flags: Flags, time: DnpTime) -> Self {
        Self { value, flags, time }
    }
}

/// Analog input measurement (Group 30 / Group 32).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Analog {
    pub value: f64,
    pub flags: Flags,
    pub time: DnpTime,
}

impl Analog {
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            flags: Flags::online(),
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags(value: f64, flags: Flags) -> Self {
        Self {
            value,
            flags,
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags_and_time(value: f64, flags: Flags, time: DnpTime) -> Self {
        Self { value, flags, time }
    }
}

/// Analog output status measurement (Group 40 / Group 42).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AnalogOutputStatus {
    pub value: f64,
    pub flags: Flags,
    pub time: DnpTime,
}

impl AnalogOutputStatus {
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            flags: Flags::online(),
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags(value: f64, flags: Flags) -> Self {
        Self {
            value,
            flags,
            time: DnpTime::invalid(),
        }
    }

    pub const fn with_flags_and_time(value: f64, flags: Flags, time: DnpTime) -> Self {
        Self { value, flags, time }
    }
}

/// Octet string measurement (Group 110 / Group 111, 1..=255 bytes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OctetString {
    pub bytes: Vec<u8>,
}

impl OctetString {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        let mut v = bytes.into();
        if v.len() > 255 {
            v.truncate(255);
        }
        Self { bytes: v }
    }
}

/// Time and Interval (Group 50 Variation 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeAndInterval {
    pub time: DnpTime,
    pub interval: u32,
    pub units: u8,
}

/// Control Relay Output Block (Group 12 Variation 1, 11 bytes on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlRelayOutputBlock {
    pub op_type: OperationType,
    pub tcc: TripCloseCode,
    pub clear: bool,
    pub count: u8,
    pub on_time_ms: u32,
    pub off_time_ms: u32,
    pub status: CommandStatus,
    pub raw_code: u8,
}

impl ControlRelayOutputBlock {
    pub const WIRE_SIZE: usize = 11;

    pub fn new(
        op_type: OperationType,
        tcc: TripCloseCode,
        clear: bool,
        count: u8,
        on_time_ms: u32,
        off_time_ms: u32,
        status: CommandStatus,
    ) -> Self {
        let mut raw_code = op_type.to_u8() & 0x0F;
        if clear {
            raw_code |= 0x20;
        }
        raw_code |= (tcc.to_u8() & 0x03) << 6;
        Self {
            op_type,
            tcc,
            clear,
            count,
            on_time_ms,
            off_time_ms,
            status,
            raw_code,
        }
    }

    pub fn from_raw_code(
        raw_code: u8,
        count: u8,
        on_time_ms: u32,
        off_time_ms: u32,
        status: CommandStatus,
    ) -> Self {
        let op_type = OperationType::from_u8(raw_code & 0x0F);
        let clear = (raw_code & 0x20) != 0;
        let tcc = TripCloseCode::from_u8((raw_code >> 6) & 0x03);
        Self {
            op_type,
            tcc,
            clear,
            count,
            on_time_ms,
            off_time_ms,
            status,
            raw_code,
        }
    }

    pub fn values_equal(&self, other: &Self) -> bool {
        self.raw_code == other.raw_code
            && self.count == other.count
            && self.on_time_ms == other.on_time_ms
            && self.off_time_ms == other.off_time_ms
    }

    pub fn is_qu_bit_set(&self) -> bool {
        (self.raw_code & 0x10) != 0
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::WIRE_SIZE {
            return None;
        }
        let raw_code = bytes[0];
        let count = bytes[1];
        let on_time_ms = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        let off_time_ms = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
        let status = CommandStatus::from_u8(bytes[10]);
        Some(Self::from_raw_code(
            raw_code,
            count,
            on_time_ms,
            off_time_ms,
            status,
        ))
    }

    pub fn to_bytes(self) -> [u8; Self::WIRE_SIZE] {
        let on = self.on_time_ms.to_le_bytes();
        let off = self.off_time_ms.to_le_bytes();
        [
            self.raw_code,
            self.count,
            on[0],
            on[1],
            on[2],
            on[3],
            off[0],
            off[1],
            off[2],
            off[3],
            self.status.to_u8(),
        ]
    }
}

/// Group 41 Variation 1 — 32-bit signed integer analog output command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalogOutputInt32 {
    pub value: i32,
    pub status: CommandStatus,
}

impl AnalogOutputInt32 {
    pub const WIRE_SIZE: usize = 5;
    pub const fn new(value: i32) -> Self {
        Self {
            value,
            status: CommandStatus::Success,
        }
    }
    pub const fn with_status(value: i32, status: CommandStatus) -> Self {
        Self { value, status }
    }
}

/// Group 41 Variation 2 — 16-bit signed integer analog output command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalogOutputInt16 {
    pub value: i16,
    pub status: CommandStatus,
}

impl AnalogOutputInt16 {
    pub const WIRE_SIZE: usize = 3;
    pub const fn new(value: i16) -> Self {
        Self {
            value,
            status: CommandStatus::Success,
        }
    }
    pub const fn with_status(value: i16, status: CommandStatus) -> Self {
        Self { value, status }
    }
}

/// Group 41 Variation 3 — 32-bit IEEE 754 float analog output command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogOutputFloat32 {
    pub value: f32,
    pub status: CommandStatus,
}

impl AnalogOutputFloat32 {
    pub const WIRE_SIZE: usize = 5;
    pub const fn new(value: f32) -> Self {
        Self {
            value,
            status: CommandStatus::Success,
        }
    }
    pub const fn with_status(value: f32, status: CommandStatus) -> Self {
        Self { value, status }
    }
}

/// Group 41 Variation 4 — 64-bit IEEE 754 double analog output command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogOutputDouble64 {
    pub value: f64,
    pub status: CommandStatus,
}

impl AnalogOutputDouble64 {
    pub const WIRE_SIZE: usize = 9;
    pub const fn new(value: f64) -> Self {
        Self {
            value,
            status: CommandStatus::Success,
        }
    }
    pub const fn with_status(value: f64, status: CommandStatus) -> Self {
        Self { value, status }
    }
}

/// Group 13 Variation 1/2 — Binary Command Event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryCommandEvent {
    pub value: bool,
    pub status: CommandStatus,
    pub time: DnpTime,
}

impl BinaryCommandEvent {
    pub const fn new(value: bool, status: CommandStatus, time: DnpTime) -> Self {
        Self {
            value,
            status,
            time,
        }
    }

    pub const fn from_flags(flags: u8, time: DnpTime) -> Self {
        Self {
            value: (flags & 0x80) != 0,
            status: CommandStatus::from_u8(flags & 0x7F),
            time,
        }
    }
}

/// Group 43 Variation 1..8 — Analog Command Event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalogCommandEvent {
    pub value: f64,
    pub status: CommandStatus,
    pub time: DnpTime,
}

impl AnalogCommandEvent {
    pub const fn new(value: f64, status: CommandStatus, time: DnpTime) -> Self {
        Self {
            value,
            status,
            time,
        }
    }
}
