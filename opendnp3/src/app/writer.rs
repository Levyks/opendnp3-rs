//! APDU serialization and object/header writing with mark/rollback support.

use super::control::AppControlField;
use super::header::{ApduHeader, ApduResponseHeader, GroupVariationId, Range};
use super::iin::IinField;
use super::measurement::{
    Analog, AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32,
    AnalogOutputStatus, Binary, BinaryOutputStatus, ControlRelayOutputBlock, Counter, DnpTime,
    DoubleBitBinary, FrozenCounter, OctetString, TimeAndInterval,
};
use crate::enums::{FunctionCode, GroupVariation, QualifierCode};

/// Mutable APDU builder backed by a bounded `Vec<u8>`.
#[derive(Debug, Clone)]
pub struct ApduWriter {
    buf: Vec<u8>,
    max_size: usize,
    is_response: bool,
}

impl ApduWriter {
    /// Start a new request APDU (`AC + FC`).
    pub fn new_request(max_size: usize, control: AppControlField, function: FunctionCode) -> Self {
        let cap = max_size.max(ApduHeader::SIZE);
        let mut buf = Vec::with_capacity(cap.min(2048));
        buf.extend_from_slice(&ApduHeader::new(control, function).to_bytes());
        Self {
            buf,
            max_size: cap,
            is_response: false,
        }
    }

    /// Start a new response APDU (`AC + FC + IIN1 + IIN2`).
    pub fn new_response(
        max_size: usize,
        control: AppControlField,
        function: FunctionCode,
        iin: IinField,
    ) -> Self {
        let cap = max_size.max(ApduResponseHeader::SIZE);
        let mut buf = Vec::with_capacity(cap.min(2048));
        buf.extend_from_slice(&ApduResponseHeader::new(control, function, iin).to_bytes());
        Self {
            buf,
            max_size: cap,
            is_response: true,
        }
    }

    pub fn set_control(&mut self, control: AppControlField) {
        self.buf[0] = control.to_byte();
    }

    pub fn set_function(&mut self, function: FunctionCode) {
        self.buf[1] = function.to_u8();
    }

    pub fn set_iin(&mut self, iin: IinField) {
        if self.is_response && self.buf.len() >= 4 {
            self.buf[2] = iin.lsb;
            self.buf[3] = iin.msb;
        }
    }

    pub fn remaining(&self) -> usize {
        self.max_size.saturating_sub(self.buf.len())
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn has_objects(&self) -> bool {
        let hdr_len = if self.is_response {
            ApduResponseHeader::SIZE
        } else {
            ApduHeader::SIZE
        };
        self.buf.len() > hdr_len
    }

    pub fn mark(&self) -> usize {
        self.buf.len()
    }

    pub fn rollback(&mut self, mark: usize) {
        self.buf.truncate(mark);
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    // ---- low-level primitives ----

    pub fn write_u8(&mut self, v: u8) -> bool {
        if self.remaining() < 1 {
            return false;
        }
        self.buf.push(v);
        true
    }

    pub fn write_u16(&mut self, v: u16) -> bool {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_i16(&mut self, v: i16) -> bool {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_u32(&mut self, v: u32) -> bool {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_i32(&mut self, v: i32) -> bool {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_f32(&mut self, v: f32) -> bool {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_f64(&mut self, v: f64) -> bool {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_time48(&mut self, t: DnpTime) -> bool {
        self.write_bytes(&t.to_le_6bytes())
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> bool {
        if self.remaining() < data.len() {
            return false;
        }
        self.buf.extend_from_slice(data);
        true
    }

    /// Patch a u8 at a previously recorded offset.
    pub fn patch_u8(&mut self, offset: usize, v: u8) {
        self.buf[offset] = v;
    }

    /// Patch a u16 (little-endian) at a previously recorded offset.
    pub fn patch_u16(&mut self, offset: usize, v: u16) {
        self.buf[offset..offset + 2].copy_from_slice(&v.to_le_bytes());
    }

    // ---- object headers ----

    pub fn write_header(&mut self, id: GroupVariationId, qualifier: QualifierCode) -> bool {
        self.write_bytes(&[id.group, id.variation, qualifier.to_u8()])
    }

    pub fn write_all_objects(&mut self, gv: GroupVariation) -> bool {
        self.write_header(
            GroupVariationId::new(gv.group(), gv.variation()),
            QualifierCode::AllObjects,
        )
    }

    pub fn write_range_header_u8(&mut self, gv: GroupVariation, start: u8, stop: u8) -> bool {
        self.write_bytes(&[
            gv.group(),
            gv.variation(),
            QualifierCode::Uint8StartStop.to_u8(),
            start,
            stop,
        ])
    }

    pub fn write_range_header_u16(&mut self, gv: GroupVariation, start: u16, stop: u16) -> bool {
        let m = self.mark();
        if self.write_bytes(&[
            gv.group(),
            gv.variation(),
            QualifierCode::Uint16StartStop.to_u8(),
        ]) && self.write_u16(start)
            && self.write_u16(stop)
        {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    pub fn write_range_header(&mut self, gv: GroupVariation, range: Range) -> bool {
        if range.is_one_byte() {
            self.write_range_header_u8(gv, range.start as u8, range.stop as u8)
        } else {
            self.write_range_header_u16(gv, range.start, range.stop)
        }
    }

    pub fn write_count_header_u8(&mut self, gv: GroupVariation, count: u8) -> bool {
        self.write_bytes(&[
            gv.group(),
            gv.variation(),
            QualifierCode::Uint8Cnt.to_u8(),
            count,
        ])
    }

    pub fn write_count_header_u16(&mut self, gv: GroupVariation, count: u16) -> bool {
        let m = self.mark();
        if self.write_bytes(&[gv.group(), gv.variation(), QualifierCode::Uint16Cnt.to_u8()])
            && self.write_u16(count)
        {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    /// Write a single IIN bit (Group 80 Var 1, 1-byte start/stop), e.g., to clear DeviceRestart (bit 7).
    pub fn write_iin_bit(&mut self, bit_index: u8, value: bool) -> bool {
        let m = self.mark();
        if self.write_range_header_u8(GroupVariation::Group80Var1, bit_index, bit_index)
            && self.write_u8(u8::from(value))
        {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    /// Write Group 50 Var 1 (Absolute Time) with 1-byte count = 1.
    pub fn write_time_g50v1(&mut self, time: DnpTime) -> bool {
        let m = self.mark();
        if self.write_count_header_u8(GroupVariation::Group50Var1, 1) && self.write_time48(time) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    /// Write Group 50 Var 3 (Last Recorded Time) with 1-byte count = 1.
    pub fn write_time_g50v3(&mut self, time: DnpTime) -> bool {
        let m = self.mark();
        if self.write_count_header_u8(GroupVariation::Group50Var3, 1) && self.write_time48(time) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    /// Write Group 52 Var 1 (Coarse time delay in seconds) or Var 2 (Fine time delay in ms).
    pub fn write_time_delay(&mut self, fine: bool, value: u16) -> bool {
        let gv = if fine {
            GroupVariation::Group52Var2
        } else {
            GroupVariation::Group52Var1
        };
        let m = self.mark();
        if self.write_count_header_u8(gv, 1) && self.write_u16(value) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    /// Write a single measurement value in the wire format of `gv` (no index prefix).
    pub fn write_binary(&mut self, gv: GroupVariation, v: &Binary) -> bool {
        match gv {
            GroupVariation::Group1Var2 | GroupVariation::Group2Var1 => {
                self.write_u8(v.wire_flags())
            }
            GroupVariation::Group2Var2 => {
                let m = self.mark();
                if self.write_u8(v.wire_flags()) && self.write_time48(v.time) {
                    true
                } else {
                    self.rollback(m);
                    false
                }
            }
            GroupVariation::Group2Var3 => {
                let m = self.mark();
                if self.write_u8(v.wire_flags()) && self.write_u16((v.time.value & 0xFFFF) as u16) {
                    true
                } else {
                    self.rollback(m);
                    false
                }
            }
            _ => false,
        }
    }

    pub fn write_double_bit(&mut self, gv: GroupVariation, v: &DoubleBitBinary) -> bool {
        match gv {
            GroupVariation::Group3Var2 | GroupVariation::Group4Var1 => {
                self.write_u8(v.wire_flags())
            }
            GroupVariation::Group4Var2 => {
                let m = self.mark();
                if self.write_u8(v.wire_flags()) && self.write_time48(v.time) {
                    true
                } else {
                    self.rollback(m);
                    false
                }
            }
            GroupVariation::Group4Var3 => {
                let m = self.mark();
                if self.write_u8(v.wire_flags()) && self.write_u16((v.time.value & 0xFFFF) as u16) {
                    true
                } else {
                    self.rollback(m);
                    false
                }
            }
            _ => false,
        }
    }

    pub fn write_binary_output_status(
        &mut self,
        gv: GroupVariation,
        v: &BinaryOutputStatus,
    ) -> bool {
        match gv {
            GroupVariation::Group10Var2 | GroupVariation::Group11Var1 => {
                self.write_u8(v.wire_flags())
            }
            GroupVariation::Group11Var2 => {
                let m = self.mark();
                if self.write_u8(v.wire_flags()) && self.write_time48(v.time) {
                    true
                } else {
                    self.rollback(m);
                    false
                }
            }
            _ => false,
        }
    }

    pub fn write_counter(&mut self, gv: GroupVariation, v: &Counter) -> bool {
        let m = self.mark();
        let ok = match gv {
            GroupVariation::Group20Var1 | GroupVariation::Group22Var1 => {
                self.write_u8(v.flags.value) && self.write_u32(v.value)
            }
            GroupVariation::Group20Var2 | GroupVariation::Group22Var2 => {
                self.write_u8(v.flags.value) && self.write_u16(v.value as u16)
            }
            GroupVariation::Group20Var5 => self.write_u32(v.value),
            GroupVariation::Group20Var6 => self.write_u16(v.value as u16),
            GroupVariation::Group22Var5 => {
                self.write_u8(v.flags.value) && self.write_u32(v.value) && self.write_time48(v.time)
            }
            GroupVariation::Group22Var6 => {
                self.write_u8(v.flags.value)
                    && self.write_u16(v.value as u16)
                    && self.write_time48(v.time)
            }
            _ => false,
        };
        if !ok {
            self.rollback(m);
        }
        ok
    }

    pub fn write_frozen_counter(&mut self, gv: GroupVariation, v: &FrozenCounter) -> bool {
        let m = self.mark();
        let ok = match gv {
            GroupVariation::Group21Var1 | GroupVariation::Group23Var1 => {
                self.write_u8(v.flags.value) && self.write_u32(v.value)
            }
            GroupVariation::Group21Var2 | GroupVariation::Group23Var2 => {
                self.write_u8(v.flags.value) && self.write_u16(v.value as u16)
            }
            GroupVariation::Group21Var5 | GroupVariation::Group23Var5 => {
                self.write_u8(v.flags.value) && self.write_u32(v.value) && self.write_time48(v.time)
            }
            GroupVariation::Group21Var6 | GroupVariation::Group23Var6 => {
                self.write_u8(v.flags.value)
                    && self.write_u16(v.value as u16)
                    && self.write_time48(v.time)
            }
            GroupVariation::Group21Var9 => self.write_u32(v.value),
            GroupVariation::Group21Var10 => self.write_u16(v.value as u16),
            _ => false,
        };
        if !ok {
            self.rollback(m);
        }
        ok
    }

    pub fn write_analog(&mut self, gv: GroupVariation, v: &Analog) -> bool {
        let m = self.mark();
        let (val_i32, over_i32) = f64_to_i32_clamped(v.value);
        let (val_i16, over_i16) = f64_to_i16_clamped(v.value);
        let flags_i32 = if over_i32 {
            v.flags.value | 0x20
        } else {
            v.flags.value
        };
        let flags_i16 = if over_i16 {
            v.flags.value | 0x20
        } else {
            v.flags.value
        };

        let ok = match gv {
            GroupVariation::Group30Var1 | GroupVariation::Group32Var1 => {
                self.write_u8(flags_i32) && self.write_i32(val_i32)
            }
            GroupVariation::Group30Var2 | GroupVariation::Group32Var2 => {
                self.write_u8(flags_i16) && self.write_i16(val_i16)
            }
            GroupVariation::Group30Var3 => self.write_i32(val_i32),
            GroupVariation::Group30Var4 => self.write_i16(val_i16),
            GroupVariation::Group30Var5 | GroupVariation::Group32Var5 => {
                self.write_u8(v.flags.value) && self.write_f32(v.value as f32)
            }
            GroupVariation::Group30Var6 | GroupVariation::Group32Var6 => {
                self.write_u8(v.flags.value) && self.write_f64(v.value)
            }
            GroupVariation::Group32Var3 => {
                self.write_u8(flags_i32) && self.write_i32(val_i32) && self.write_time48(v.time)
            }
            GroupVariation::Group32Var4 => {
                self.write_u8(flags_i16) && self.write_i16(val_i16) && self.write_time48(v.time)
            }
            GroupVariation::Group32Var7 => {
                self.write_u8(v.flags.value)
                    && self.write_f32(v.value as f32)
                    && self.write_time48(v.time)
            }
            GroupVariation::Group32Var8 => {
                self.write_u8(v.flags.value) && self.write_f64(v.value) && self.write_time48(v.time)
            }
            _ => false,
        };
        if !ok {
            self.rollback(m);
        }
        ok
    }

    pub fn write_analog_output_status(
        &mut self,
        gv: GroupVariation,
        v: &AnalogOutputStatus,
    ) -> bool {
        let m = self.mark();
        let (val_i32, over_i32) = f64_to_i32_clamped(v.value);
        let (val_i16, over_i16) = f64_to_i16_clamped(v.value);
        let flags_i32 = if over_i32 {
            v.flags.value | 0x20
        } else {
            v.flags.value
        };
        let flags_i16 = if over_i16 {
            v.flags.value | 0x20
        } else {
            v.flags.value
        };

        let ok = match gv {
            GroupVariation::Group40Var1 | GroupVariation::Group42Var1 => {
                self.write_u8(flags_i32) && self.write_i32(val_i32)
            }
            GroupVariation::Group40Var2 | GroupVariation::Group42Var2 => {
                self.write_u8(flags_i16) && self.write_i16(val_i16)
            }
            GroupVariation::Group40Var3 | GroupVariation::Group42Var5 => {
                self.write_u8(v.flags.value) && self.write_f32(v.value as f32)
            }
            GroupVariation::Group40Var4 | GroupVariation::Group42Var6 => {
                self.write_u8(v.flags.value) && self.write_f64(v.value)
            }
            GroupVariation::Group42Var3 => {
                self.write_u8(flags_i32) && self.write_i32(val_i32) && self.write_time48(v.time)
            }
            GroupVariation::Group42Var4 => {
                self.write_u8(flags_i16) && self.write_i16(val_i16) && self.write_time48(v.time)
            }
            GroupVariation::Group42Var7 => {
                self.write_u8(v.flags.value)
                    && self.write_f32(v.value as f32)
                    && self.write_time48(v.time)
            }
            GroupVariation::Group42Var8 => {
                self.write_u8(v.flags.value) && self.write_f64(v.value) && self.write_time48(v.time)
            }
            _ => false,
        };
        if !ok {
            self.rollback(m);
        }
        ok
    }

    pub fn write_crob(&mut self, v: &ControlRelayOutputBlock) -> bool {
        self.write_bytes(&v.to_bytes())
    }

    pub fn write_ao_i32(&mut self, v: &AnalogOutputInt32) -> bool {
        let m = self.mark();
        if self.write_i32(v.value) && self.write_u8(v.status.to_u8()) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    pub fn write_ao_i16(&mut self, v: &AnalogOutputInt16) -> bool {
        let m = self.mark();
        if self.write_i16(v.value) && self.write_u8(v.status.to_u8()) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    pub fn write_ao_f32(&mut self, v: &AnalogOutputFloat32) -> bool {
        let m = self.mark();
        if self.write_f32(v.value) && self.write_u8(v.status.to_u8()) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    pub fn write_ao_f64(&mut self, v: &AnalogOutputDouble64) -> bool {
        let m = self.mark();
        if self.write_f64(v.value) && self.write_u8(v.status.to_u8()) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    pub fn write_time_and_interval(&mut self, v: &TimeAndInterval) -> bool {
        let m = self.mark();
        if self.write_time48(v.time) && self.write_u32(v.interval) && self.write_u8(v.units) {
            true
        } else {
            self.rollback(m);
            false
        }
    }

    /// Write packed single-bit binaries (Group 1 Var 1) over `[start..=stop]`.
    pub fn write_packed_binaries(&mut self, start: u16, values: &[bool]) -> bool {
        if values.is_empty() {
            return true;
        }
        let stop = match start.checked_add((values.len() - 1) as u16) {
            Some(s) => s,
            None => return false,
        };
        let m = self.mark();
        if !self.write_range_header(GroupVariation::Group1Var1, Range::new(start, stop)) {
            return false;
        }
        let num_bytes = values.len().div_ceil(8);
        if self.remaining() < num_bytes {
            self.rollback(m);
            return false;
        }
        for chunk in values.chunks(8) {
            let mut byte = 0u8;
            for (bit, &val) in chunk.iter().enumerate() {
                if val {
                    byte |= 1 << bit;
                }
            }
            self.buf.push(byte);
        }
        true
    }

    /// Write a single prefixed octet string (Group 110 or 111, variation = len).
    pub fn write_octet_string_prefixed(&mut self, group: u8, index: u16, s: &OctetString) -> bool {
        if s.bytes.is_empty() || s.bytes.len() > 255 {
            return false;
        }
        let m = self.mark();
        let ok = self.write_bytes(&[
            group,
            s.bytes.len() as u8,
            QualifierCode::Uint16CntUint16Index.to_u8(),
        ]) && self.write_u16(1)
            && self.write_u16(index)
            && self.write_bytes(&s.bytes);
        if !ok {
            self.rollback(m);
        }
        ok
    }
}

fn f64_to_i32_clamped(v: f64) -> (i32, bool) {
    if v > i32::MAX as f64 {
        (i32::MAX, true)
    } else if v < i32::MIN as f64 {
        (i32::MIN, true)
    } else {
        (v as i32, false)
    }
}

fn f64_to_i16_clamped(v: f64) -> (i16, bool) {
    if v > i16::MAX as f64 {
        (i16::MAX, true)
    } else if v < i16::MIN as f64 {
        (i16::MIN, true)
    } else {
        (v as i16, false)
    }
}
