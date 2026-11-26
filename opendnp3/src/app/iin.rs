//! Internal Indications (IIN) 2-byte bitfield present in every response APDU.

/// Individual Internal Indication bits (IIN1 = bits 0..7, IIN2 = bits 8..15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IinBit {
    // IIN1 (LSB)
    AllStations = 0,
    Class1Events = 1,
    Class2Events = 2,
    Class3Events = 3,
    NeedTime = 4,
    LocalControl = 5,
    DeviceTrouble = 6,
    DeviceRestart = 7,
    // IIN2 (MSB)
    FuncNotSupported = 8,
    ObjectUnknown = 9,
    ParamError = 10,
    EventBufferOverflow = 11,
    AlreadyExecuting = 12,
    ConfigCorrupt = 13,
    Reserved1 = 14,
    Reserved2 = 15,
}

/// 16-bit Internal Indications field (`lsb` = IIN1, `msb` = IIN2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IinField {
    pub lsb: u8,
    pub msb: u8,
}

impl IinField {
    pub const fn empty() -> Self {
        Self { lsb: 0, msb: 0 }
    }

    pub const fn new(lsb: u8, msb: u8) -> Self {
        Self { lsb, msb }
    }

    pub const fn from_bit(bit: IinBit) -> Self {
        let idx = bit as u8;
        if idx < 8 {
            Self {
                lsb: 1 << idx,
                msb: 0,
            }
        } else {
            Self {
                lsb: 0,
                msb: 1 << (idx - 8),
            }
        }
    }

    pub const fn is_set(self, bit: IinBit) -> bool {
        let idx = bit as u8;
        if idx < 8 {
            (self.lsb & (1 << idx)) != 0
        } else {
            (self.msb & (1 << (idx - 8))) != 0
        }
    }

    pub fn set_bit(&mut self, bit: IinBit) {
        let idx = bit as u8;
        if idx < 8 {
            self.lsb |= 1 << idx;
        } else {
            self.msb |= 1 << (idx - 8);
        }
    }

    pub fn clear_bit(&mut self, bit: IinBit) {
        let idx = bit as u8;
        if idx < 8 {
            self.lsb &= !(1 << idx);
        } else {
            self.msb &= !(1 << (idx - 8));
        }
    }

    pub fn set_bit_to(&mut self, bit: IinBit, value: bool) {
        if value {
            self.set_bit(bit);
        } else {
            self.clear_bit(bit);
        }
    }

    pub const fn any(self) -> bool {
        (self.lsb | self.msb) != 0
    }

    pub const fn has_request_error(self) -> bool {
        (self.msb & 0x07) != 0 // FuncNotSupported | ObjectUnknown | ParamError
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            lsb: self.lsb | other.lsb,
            msb: self.msb | other.msb,
        }
    }
}

impl core::ops::BitOr for IinField {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl core::ops::BitOrAssign for IinField {
    fn bitor_assign(&mut self, rhs: Self) {
        self.lsb |= rhs.lsb;
        self.msb |= rhs.msb;
    }
}
