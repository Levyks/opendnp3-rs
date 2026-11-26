//! Quality flag bit definitions and DoubleBit state enumeration.

/// Binary input quality flags (Group 1 / Group 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BinaryQuality {
    Online = 0x01,
    Restart = 0x02,
    CommLost = 0x04,
    RemoteForced = 0x08,
    LocalForced = 0x10,
    ChatterFilter = 0x20,
    Reserved = 0x40,
    State = 0x80,
}

/// Double-bit binary input quality flags (Group 3 / Group 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DoubleBitBinaryQuality {
    Online = 0x01,
    Restart = 0x02,
    CommLost = 0x04,
    RemoteForced = 0x08,
    LocalForced = 0x10,
    ChatterFilter = 0x20,
    State1 = 0x40,
    State2 = 0x80,
}

/// Binary output status quality flags (Group 10 / Group 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BinaryOutputStatusQuality {
    Online = 0x01,
    Restart = 0x02,
    CommLost = 0x04,
    RemoteForced = 0x08,
    LocalForced = 0x10,
    Reserved1 = 0x20,
    Reserved2 = 0x40,
    State = 0x80,
}

/// Counter quality flags (Group 20 / Group 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CounterQuality {
    Online = 0x01,
    Restart = 0x02,
    CommLost = 0x04,
    RemoteForced = 0x08,
    LocalForced = 0x10,
    Rollover = 0x20,
    Discontinuity = 0x40,
    Reserved = 0x80,
}

/// Frozen counter quality flags (Group 21 / Group 23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FrozenCounterQuality {
    Online = 0x01,
    Restart = 0x02,
    CommLost = 0x04,
    RemoteForced = 0x08,
    LocalForced = 0x10,
    Rollover = 0x20,
    Discontinuity = 0x40,
    Reserved = 0x80,
}

/// Analog input quality flags (Group 30 / Group 32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AnalogQuality {
    Online = 0x01,
    Restart = 0x02,
    CommLost = 0x04,
    RemoteForced = 0x08,
    LocalForced = 0x10,
    OverRange = 0x20,
    ReferenceErr = 0x40,
    Reserved = 0x80,
}

/// Analog output status quality flags (Group 40 / Group 42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AnalogOutputStatusQuality {
    Online = 0x01,
    Restart = 0x02,
    CommLost = 0x04,
    RemoteForced = 0x08,
    LocalForced = 0x10,
    OverRange = 0x20,
    ReferenceErr = 0x40,
    Reserved = 0x80,
}

/// Double-bit binary value (2-bit state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DoubleBit {
    #[default]
    Intermediate = 0x00,
    DeterminedOff = 0x01,
    DeterminedOn = 0x02,
    Indeterminate = 0x03,
}

impl DoubleBit {
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    pub const fn from_u8(value: u8) -> Self {
        match value & 0x03 {
            0x00 => Self::Intermediate,
            0x01 => Self::DeterminedOff,
            0x02 => Self::DeterminedOn,
            _ => Self::Indeterminate,
        }
    }
}
