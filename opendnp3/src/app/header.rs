//! Application-layer headers, class bitmasks, and object header metadata.

use super::control::AppControlField;
use super::iin::IinField;
use crate::enums::{EventClass, FunctionCode, GroupVariation, PointClass, QualifierCode};

/// Request APDU header (2 bytes: AC + FC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApduHeader {
    pub control: AppControlField,
    pub function: FunctionCode,
}

impl ApduHeader {
    pub const SIZE: usize = 2;

    pub const fn new(control: AppControlField, function: FunctionCode) -> Self {
        Self { control, function }
    }

    pub const fn confirm(seq: u8, unsolicited: bool) -> Self {
        Self {
            control: AppControlField::new(true, true, false, unsolicited, seq),
            function: FunctionCode::Confirm,
        }
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            control: AppControlField::from_byte(bytes[0]),
            function: FunctionCode::from_u8(bytes[1]),
        })
    }

    pub const fn to_bytes(self) -> [u8; 2] {
        [self.control.to_byte(), self.function.to_u8()]
    }
}

/// Response APDU header (4 bytes: AC + FC + IIN1 + IIN2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApduResponseHeader {
    pub control: AppControlField,
    pub function: FunctionCode,
    pub iin: IinField,
}

impl ApduResponseHeader {
    pub const SIZE: usize = 4;

    pub const fn new(control: AppControlField, function: FunctionCode, iin: IinField) -> Self {
        Self {
            control,
            function,
            iin,
        }
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            control: AppControlField::from_byte(bytes[0]),
            function: FunctionCode::from_u8(bytes[1]),
            iin: IinField::new(bytes[2], bytes[3]),
        })
    }

    pub const fn to_bytes(self) -> [u8; 4] {
        [
            self.control.to_byte(),
            self.function.to_u8(),
            self.iin.lsb,
            self.iin.msb,
        ]
    }
}

/// Inclusive index range `[start, stop]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Range {
    pub start: u16,
    pub stop: u16,
}

impl Range {
    pub const fn new(start: u16, stop: u16) -> Self {
        Self { start, stop }
    }

    pub const fn is_valid(self) -> bool {
        self.start <= self.stop
    }

    pub const fn count(self) -> usize {
        if self.start <= self.stop {
            (self.stop as usize - self.start as usize) + 1
        } else {
            0
        }
    }

    pub const fn is_one_byte(self) -> bool {
        self.start <= 255 && self.stop <= 255
    }
}

/// Raw `(group, variation)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupVariationId {
    pub group: u8,
    pub variation: u8,
}

impl GroupVariationId {
    pub const fn new(group: u8, variation: u8) -> Self {
        Self { group, variation }
    }

    pub const fn to_enum(self) -> GroupVariation {
        GroupVariation::from_group_var(self.group, self.variation)
    }
}

/// Parsed 3-byte object header (`group`, `variation`, `qualifier`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectHeader {
    pub group: u8,
    pub variation: u8,
    pub qualifier: QualifierCode,
    pub raw_qualifier: u8,
}

impl ObjectHeader {
    pub const fn gv(&self) -> GroupVariation {
        GroupVariation::from_group_var(self.group, self.variation)
    }

    pub const fn id(&self) -> GroupVariationId {
        GroupVariationId::new(self.group, self.variation)
    }
}

/// Metadata passed to handlers for each parsed object header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderInfo {
    pub gv: GroupVariation,
    pub group: u8,
    pub variation: u8,
    pub qualifier: QualifierCode,
    pub header_index: usize,
}

/// Bitmask of DNP3 classes (Class 0, 1, 2, 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClassField(u8);

impl ClassField {
    pub const NONE: Self = Self(0);
    pub const CLASS_0: Self = Self(PointClass::Class0 as u8);
    pub const CLASS_1: Self = Self(PointClass::Class1 as u8);
    pub const CLASS_2: Self = Self(PointClass::Class2 as u8);
    pub const CLASS_3: Self = Self(PointClass::Class3 as u8);
    pub const ALL_EVENT_CLASSES: Self =
        Self((PointClass::Class1 as u8) | (PointClass::Class2 as u8) | (PointClass::Class3 as u8));
    pub const ALL_CLASSES: Self = Self(0x0F);

    pub const fn new(mask: u8) -> Self {
        Self(mask & 0x0F)
    }

    pub const fn from_point_class(pc: PointClass) -> Self {
        Self(pc as u8)
    }

    pub const fn from_event_class(ec: EventClass) -> Self {
        Self(ec.to_point_class() as u8)
    }

    pub const fn has_class0(self) -> bool {
        (self.0 & PointClass::Class0 as u8) != 0
    }
    pub const fn has_class1(self) -> bool {
        (self.0 & PointClass::Class1 as u8) != 0
    }
    pub const fn has_class2(self) -> bool {
        (self.0 & PointClass::Class2 as u8) != 0
    }
    pub const fn has_class3(self) -> bool {
        (self.0 & PointClass::Class3 as u8) != 0
    }
    pub const fn has_event_type(self, ec: EventClass) -> bool {
        (self.0 & ec.to_point_class() as u8) != 0
    }
    pub const fn has_any_class(self) -> bool {
        self.0 != 0
    }
    pub const fn has_event_class(self) -> bool {
        (self.0 & Self::ALL_EVENT_CLASSES.0) != 0
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn set(&mut self, other: Self) {
        self.0 |= other.0;
    }
    pub fn clear(&mut self, other: Self) {
        self.0 &= !other.0;
    }
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
