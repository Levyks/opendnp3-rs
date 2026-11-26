//! APDU object-header and payload parser.

use super::header::{HeaderInfo, ObjectHeader, Range};
use super::iin::{IinBit, IinField};
use super::measurement::{
    Analog, AnalogCommandEvent, AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16,
    AnalogOutputInt32, AnalogOutputStatus, Binary, BinaryCommandEvent, BinaryOutputStatus,
    ControlRelayOutputBlock, Counter, DnpTime, DoubleBitBinary, Flags, FrozenCounter, Indexed,
    OctetString, TimeAndInterval,
};
use crate::enums::{CommandStatus, DoubleBit, GroupVariation, QualifierCode, TimestampQuality};
use crate::util::ReadCursor;

/// Result of parsing an APDU's object headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseResult {
    Ok,
    NotEnoughDataForHeader,
    NotEnoughDataForRange,
    NotEnoughDataForObjects,
    MalformedFreeFormat,
    UnknownObject,
    UnknownQualifier,
    InvalidObjectQualifier,
    InvalidObject,
    BadStartStop,
    CountOfZero,
}

impl ParseResult {
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    pub const fn to_iin(self) -> IinField {
        match self {
            Self::Ok => IinField::empty(),
            Self::UnknownObject | Self::UnknownQualifier => {
                IinField::from_bit(IinBit::ObjectUnknown)
            }
            _ => IinField::from_bit(IinBit::ParamError),
        }
    }
}

/// Visitor trait invoked by [`parse_objects`] for each decoded header.
///
/// Default implementations return `IinField::from_bit(IinBit::ObjectUnknown)` so
/// callers only override the headers they handle.
#[allow(unused_variables)]
pub trait ApduHandler {
    fn on_all_objects(&mut self, info: HeaderInfo) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_range_header(&mut self, info: HeaderInfo, range: Range) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_count_header(&mut self, info: HeaderInfo, count: u16) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }

    // Decoded value callbacks
    fn on_binaries(&mut self, info: HeaderInfo, values: &[Indexed<Binary>]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_double_bits(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<DoubleBitBinary>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_binary_output_statuses(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<BinaryOutputStatus>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_counters(&mut self, info: HeaderInfo, values: &[Indexed<Counter>]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_frozen_counters(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<FrozenCounter>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_analogs(&mut self, info: HeaderInfo, values: &[Indexed<Analog>]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_analog_output_statuses(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputStatus>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_octet_strings(&mut self, info: HeaderInfo, values: &[Indexed<OctetString>]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_time_and_intervals(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<TimeAndInterval>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_crobs(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<ControlRelayOutputBlock>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_ao_i32s(&mut self, info: HeaderInfo, values: &[Indexed<AnalogOutputInt32>]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_ao_i16s(&mut self, info: HeaderInfo, values: &[Indexed<AnalogOutputInt16>]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_ao_f32s(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputFloat32>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_ao_f64s(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogOutputDouble64>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_iin_bits(&mut self, info: HeaderInfo, values: &[Indexed<bool>]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_time_g50v1(&mut self, info: HeaderInfo, values: &[DnpTime]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_time_g50v3(&mut self, info: HeaderInfo, values: &[DnpTime]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_time_delay(&mut self, info: HeaderInfo, fine: bool, values: &[u16]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_binary_command_events(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<BinaryCommandEvent>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_analog_command_events(
        &mut self,
        info: HeaderInfo,
        values: &[Indexed<AnalogCommandEvent>],
    ) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
    fn on_indices(&mut self, info: HeaderInfo, indices: &[u16]) -> IinField {
        IinField::from_bit(IinBit::ObjectUnknown)
    }
}

/// Whether object headers are expected to carry payload bytes (e.g. responses,
/// WRITE, SELECT, OPERATE) or header-only specifiers (e.g. READ, FREEZE,
/// ENABLE/DISABLE UNSOLICITED, ASSIGN_CLASS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseMode {
    /// Headers carry object data (responses, WRITE, SELECT, OPERATE, etc.).
    WithObjects,
    /// Headers specify ranges/counts without object payloads (READ, FREEZE, etc.).
    HeaderOnly,
}

/// Parse all object headers in `bytes` (the APDU slice *after* the 2- or 4-byte APDU header).
/// Returns `(ParseResult, accumulated_iin)`.
pub fn parse_objects(
    bytes: &[u8],
    mode: ParseMode,
    handler: &mut dyn ApduHandler,
) -> (ParseResult, IinField) {
    let mut cursor = ReadCursor::new(bytes);
    let mut header_index = 0usize;
    let mut iin = IinField::empty();
    let mut cto: Option<DnpTime> = None;

    while !cursor.is_empty() {
        let Some(hdr) = read_object_header(&mut cursor) else {
            return (ParseResult::NotEnoughDataForHeader, iin);
        };
        let info = HeaderInfo {
            gv: hdr.gv(),
            group: hdr.group,
            variation: hdr.variation,
            qualifier: hdr.qualifier,
            header_index,
        };
        header_index += 1;

        match hdr.qualifier {
            QualifierCode::AllObjects => {
                if mode == ParseMode::WithObjects && !is_header_only_gv(hdr) {
                    // AllObjects never carries inline payload
                    iin |= handler.on_all_objects(info);
                } else {
                    iin |= handler.on_all_objects(info);
                }
            }
            QualifierCode::Uint8StartStop | QualifierCode::Uint16StartStop => {
                let range = match hdr.qualifier {
                    QualifierCode::Uint8StartStop => {
                        let (Ok(s), Ok(e)) = (cursor.read_u8(), cursor.read_u8()) else {
                            return (ParseResult::NotEnoughDataForRange, iin);
                        };
                        Range::new(s as u16, e as u16)
                    }
                    _ => {
                        let (Ok(s), Ok(e)) = (cursor.read_u16_le(), cursor.read_u16_le()) else {
                            return (ParseResult::NotEnoughDataForRange, iin);
                        };
                        Range::new(s, e)
                    }
                };
                if !range.is_valid() {
                    return (ParseResult::BadStartStop, iin);
                }
                if mode == ParseMode::HeaderOnly {
                    iin |= handler.on_range_header(info, range);
                } else {
                    match parse_range_objects(&mut cursor, info, range, cto, handler) {
                        Ok(h_iin) => iin |= h_iin,
                        Err(e) => return (e, iin),
                    }
                }
            }
            QualifierCode::Uint8Cnt | QualifierCode::Uint16Cnt => {
                let count = match hdr.qualifier {
                    QualifierCode::Uint8Cnt => {
                        let Ok(c) = cursor.read_u8() else {
                            return (ParseResult::NotEnoughDataForRange, iin);
                        };
                        c as u16
                    }
                    _ => {
                        let Ok(c) = cursor.read_u16_le() else {
                            return (ParseResult::NotEnoughDataForRange, iin);
                        };
                        c
                    }
                };
                if count == 0 {
                    return (ParseResult::CountOfZero, iin);
                }
                if mode == ParseMode::HeaderOnly {
                    iin |= handler.on_count_header(info, count);
                } else {
                    match parse_count_objects(&mut cursor, info, count, &mut cto, handler) {
                        Ok(h_iin) => iin |= h_iin,
                        Err(e) => return (e, iin),
                    }
                }
            }
            QualifierCode::Uint8CntUint8Index | QualifierCode::Uint16CntUint16Index => {
                let index_u16 = hdr.qualifier == QualifierCode::Uint16CntUint16Index;
                let count = if index_u16 {
                    let Ok(c) = cursor.read_u16_le() else {
                        return (ParseResult::NotEnoughDataForRange, iin);
                    };
                    c
                } else {
                    let Ok(c) = cursor.read_u8() else {
                        return (ParseResult::NotEnoughDataForRange, iin);
                    };
                    c as u16
                };
                if count == 0 {
                    return (ParseResult::CountOfZero, iin);
                }
                if mode == ParseMode::HeaderOnly {
                    let mut indices = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        match read_index(&mut cursor, index_u16) {
                            Ok(idx) => indices.push(idx),
                            Err(e) => return (e, iin),
                        }
                    }
                    iin |= handler.on_indices(info, &indices);
                } else {
                    match parse_prefixed_objects(&mut cursor, info, count, index_u16, cto, handler)
                    {
                        Ok(h_iin) => iin |= h_iin,
                        Err(e) => return (e, iin),
                    }
                }
            }
            QualifierCode::Undefined | QualifierCode::Uint16FreeFormat => {
                return (ParseResult::UnknownQualifier, iin);
            }
        }
    }

    (ParseResult::Ok, iin)
}

fn read_object_header(cursor: &mut ReadCursor<'_>) -> Option<ObjectHeader> {
    let group = cursor.read_u8().ok()?;
    let variation = cursor.read_u8().ok()?;
    let raw_qualifier = cursor.read_u8().ok()?;
    Some(ObjectHeader {
        group,
        variation,
        qualifier: QualifierCode::from_u8(raw_qualifier),
        raw_qualifier,
    })
}

fn is_header_only_gv(hdr: ObjectHeader) -> bool {
    hdr.group == 60
}

fn read_time48(cursor: &mut ReadCursor<'_>) -> Result<DnpTime, ParseResult> {
    let bytes = cursor
        .read_bytes(6)
        .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
    let arr: [u8; 6] = bytes.try_into().unwrap();
    Ok(DnpTime::from_le_6bytes(arr))
}

fn resolve_cto_time(cto: Option<DnpTime>, rel_ms: u16) -> DnpTime {
    match cto {
        Some(base) => DnpTime::with_quality(base.value.saturating_add(rel_ms as u64), base.quality),
        None => DnpTime::with_quality(rel_ms as u64, TimestampQuality::Invalid),
    }
}

fn parse_range_objects(
    cursor: &mut ReadCursor<'_>,
    info: HeaderInfo,
    range: Range,
    cto: Option<DnpTime>,
    handler: &mut dyn ApduHandler,
) -> Result<IinField, ParseResult> {
    let count = range.count();

    // Octet string static (Group 110, variation = byte length 1..=255)
    if info.group == 110 {
        if info.variation == 0 {
            return Err(ParseResult::InvalidObject);
        }
        let len = info.variation as usize;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let slice = cursor
                .read_bytes(len)
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            out.push(Indexed::new(
                range.start + i as u16,
                OctetString::new(slice),
            ));
        }
        return Ok(handler.on_octet_strings(info, &out));
    }

    match info.gv {
        // Packed single-bit binaries (G1V1)
        GroupVariation::Group1Var1 => {
            let num_bytes = count.div_ceil(8);
            let bytes = cursor
                .read_bytes(num_bytes)
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            let mut out = Vec::with_capacity(count);
            for i in 0..count {
                let bit = (bytes[i / 8] >> (i % 8)) & 1 != 0;
                out.push(Indexed::new(range.start + i as u16, Binary::new(bit)));
            }
            Ok(handler.on_binaries(info, &out))
        }
        // Packed double-bit binaries (G3V1, 2 bits per point)
        GroupVariation::Group3Var1 => {
            let num_bytes = count.div_ceil(4);
            let bytes = cursor
                .read_bytes(num_bytes)
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            let mut out = Vec::with_capacity(count);
            for i in 0..count {
                let bits = (bytes[i / 4] >> ((i % 4) * 2)) & 0x03;
                out.push(Indexed::new(
                    range.start + i as u16,
                    DoubleBitBinary::new(DoubleBit::from_u8(bits)),
                ));
            }
            Ok(handler.on_double_bits(info, &out))
        }
        // Packed binary output status (G10V1)
        GroupVariation::Group10Var1 => {
            let num_bytes = count.div_ceil(8);
            let bytes = cursor
                .read_bytes(num_bytes)
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            let mut out = Vec::with_capacity(count);
            for i in 0..count {
                let bit = (bytes[i / 8] >> (i % 8)) & 1 != 0;
                out.push(Indexed::new(
                    range.start + i as u16,
                    BinaryOutputStatus::new(bit),
                ));
            }
            Ok(handler.on_binary_output_statuses(info, &out))
        }
        // Packed IIN bits (G80V1)
        GroupVariation::Group80Var1 => {
            let num_bytes = count.div_ceil(8);
            let bytes = cursor
                .read_bytes(num_bytes)
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            let mut out = Vec::with_capacity(count);
            for i in 0..count {
                let bit = (bytes[i / 8] >> (i % 8)) & 1 != 0;
                out.push(Indexed::new(range.start + i as u16, bit));
            }
            Ok(handler.on_iin_bits(info, &out))
        }
        _ => {
            let indices: Vec<u16> = (0..count).map(|i| range.start + i as u16).collect();
            parse_indexed_values(cursor, info, &indices, cto, handler)
        }
    }
}

fn parse_count_objects(
    cursor: &mut ReadCursor<'_>,
    info: HeaderInfo,
    count: u16,
    cto: &mut Option<DnpTime>,
    handler: &mut dyn ApduHandler,
) -> Result<IinField, ParseResult> {
    let n = count as usize;
    match info.gv {
        GroupVariation::Group50Var1 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(read_time48(cursor)?);
            }
            Ok(handler.on_time_g50v1(info, &out))
        }
        GroupVariation::Group50Var3 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(read_time48(cursor)?);
            }
            Ok(handler.on_time_g50v3(info, &out))
        }
        GroupVariation::Group51Var1 => {
            for _ in 0..n {
                let t = read_time48(cursor)?;
                *cto = Some(DnpTime::with_quality(
                    t.value,
                    TimestampQuality::Synchronized,
                ));
            }
            Ok(IinField::empty())
        }
        GroupVariation::Group51Var2 => {
            for _ in 0..n {
                let t = read_time48(cursor)?;
                *cto = Some(DnpTime::with_quality(
                    t.value,
                    TimestampQuality::Unsynchronized,
                ));
            }
            Ok(IinField::empty())
        }
        GroupVariation::Group52Var1 | GroupVariation::Group52Var2 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let v = cursor
                    .read_u16_le()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                out.push(v);
            }
            Ok(handler.on_time_delay(info, info.gv == GroupVariation::Group52Var2, &out))
        }
        _ => Err(ParseResult::InvalidObjectQualifier),
    }
}

fn parse_prefixed_objects(
    cursor: &mut ReadCursor<'_>,
    info: HeaderInfo,
    count: u16,
    index_u16: bool,
    cto: Option<DnpTime>,
    handler: &mut dyn ApduHandler,
) -> Result<IinField, ParseResult> {
    let n = count as usize;

    // Octet string events (Group 110 / 111, variation = length)
    if (info.group == 110 || info.group == 111) && info.variation > 0 {
        let len = info.variation as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let idx = read_index(cursor, index_u16)?;
            let slice = cursor
                .read_bytes(len)
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            out.push(Indexed::new(idx, OctetString::new(slice)));
        }
        return Ok(handler.on_octet_strings(info, &out));
    }

    match info.gv {
        GroupVariation::Group2Var1 | GroupVariation::Group2Var2 | GroupVariation::Group2Var3 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(idx, read_binary(cursor, info.gv, cto)?));
            }
            Ok(handler.on_binaries(info, &out))
        }
        GroupVariation::Group4Var1 | GroupVariation::Group4Var2 | GroupVariation::Group4Var3 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(idx, read_double_bit(cursor, info.gv, cto)?));
            }
            Ok(handler.on_double_bits(info, &out))
        }
        GroupVariation::Group11Var1 | GroupVariation::Group11Var2 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(
                    idx,
                    read_binary_output_status(cursor, info.gv)?,
                ));
            }
            Ok(handler.on_binary_output_statuses(info, &out))
        }
        GroupVariation::Group12Var1 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let raw = cursor
                    .read_bytes(ControlRelayOutputBlock::WIRE_SIZE)
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                let crob = ControlRelayOutputBlock::from_bytes(raw)
                    .ok_or(ParseResult::NotEnoughDataForObjects)?;
                out.push(Indexed::new(idx, crob));
            }
            Ok(handler.on_crobs(info, &out))
        }
        GroupVariation::Group22Var1
        | GroupVariation::Group22Var2
        | GroupVariation::Group22Var5
        | GroupVariation::Group22Var6 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(idx, read_counter(cursor, info.gv)?));
            }
            Ok(handler.on_counters(info, &out))
        }
        GroupVariation::Group23Var1
        | GroupVariation::Group23Var2
        | GroupVariation::Group23Var5
        | GroupVariation::Group23Var6 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(idx, read_frozen_counter(cursor, info.gv)?));
            }
            Ok(handler.on_frozen_counters(info, &out))
        }
        GroupVariation::Group32Var1
        | GroupVariation::Group32Var2
        | GroupVariation::Group32Var3
        | GroupVariation::Group32Var4
        | GroupVariation::Group32Var5
        | GroupVariation::Group32Var6
        | GroupVariation::Group32Var7
        | GroupVariation::Group32Var8 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(idx, read_analog(cursor, info.gv)?));
            }
            Ok(handler.on_analogs(info, &out))
        }
        GroupVariation::Group41Var1 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let val = cursor
                    .read_i32_le()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                let st = CommandStatus::from_u8(
                    cursor
                        .read_u8()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
                );
                out.push(Indexed::new(idx, AnalogOutputInt32::with_status(val, st)));
            }
            Ok(handler.on_ao_i32s(info, &out))
        }
        GroupVariation::Group41Var2 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let val = cursor
                    .read_i16_le()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                let st = CommandStatus::from_u8(
                    cursor
                        .read_u8()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
                );
                out.push(Indexed::new(idx, AnalogOutputInt16::with_status(val, st)));
            }
            Ok(handler.on_ao_i16s(info, &out))
        }
        GroupVariation::Group41Var3 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let val = cursor
                    .read_f32_le()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                let st = CommandStatus::from_u8(
                    cursor
                        .read_u8()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
                );
                out.push(Indexed::new(idx, AnalogOutputFloat32::with_status(val, st)));
            }
            Ok(handler.on_ao_f32s(info, &out))
        }
        GroupVariation::Group41Var4 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let val = cursor
                    .read_f64_le()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                let st = CommandStatus::from_u8(
                    cursor
                        .read_u8()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
                );
                out.push(Indexed::new(
                    idx,
                    AnalogOutputDouble64::with_status(val, st),
                ));
            }
            Ok(handler.on_ao_f64s(info, &out))
        }
        GroupVariation::Group42Var1
        | GroupVariation::Group42Var2
        | GroupVariation::Group42Var3
        | GroupVariation::Group42Var4
        | GroupVariation::Group42Var5
        | GroupVariation::Group42Var6
        | GroupVariation::Group42Var7
        | GroupVariation::Group42Var8 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(
                    idx,
                    read_analog_output_status(cursor, info.gv)?,
                ));
            }
            Ok(handler.on_analog_output_statuses(info, &out))
        }
        GroupVariation::Group13Var1 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let flags = cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                out.push(Indexed::new(
                    idx,
                    BinaryCommandEvent::from_flags(flags, DnpTime::invalid()),
                ));
            }
            Ok(handler.on_binary_command_events(info, &out))
        }
        GroupVariation::Group13Var2 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let flags = cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
                let t = read_time48(cursor)?;
                out.push(Indexed::new(idx, BinaryCommandEvent::from_flags(flags, t)));
            }
            Ok(handler.on_binary_command_events(info, &out))
        }
        GroupVariation::Group43Var1
        | GroupVariation::Group43Var2
        | GroupVariation::Group43Var3
        | GroupVariation::Group43Var4
        | GroupVariation::Group43Var5
        | GroupVariation::Group43Var6
        | GroupVariation::Group43Var7
        | GroupVariation::Group43Var8 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                let status = CommandStatus::from_u8(
                    cursor
                        .read_u8()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
                );
                let val = match info.gv {
                    GroupVariation::Group43Var1 | GroupVariation::Group43Var3 => cursor
                        .read_i32_le()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?
                        as f64,
                    GroupVariation::Group43Var2 | GroupVariation::Group43Var4 => cursor
                        .read_i16_le()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?
                        as f64,
                    GroupVariation::Group43Var5 | GroupVariation::Group43Var7 => cursor
                        .read_f32_le()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?
                        as f64,
                    _ => cursor
                        .read_f64_le()
                        .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
                };
                let time = match info.gv {
                    GroupVariation::Group43Var3
                    | GroupVariation::Group43Var4
                    | GroupVariation::Group43Var7
                    | GroupVariation::Group43Var8 => read_time48(cursor)?,
                    _ => DnpTime::invalid(),
                };
                out.push(Indexed::new(
                    idx,
                    AnalogCommandEvent::new(val, status, time),
                ));
            }
            Ok(handler.on_analog_command_events(info, &out))
        }
        GroupVariation::Group50Var4 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let idx = read_index(cursor, index_u16)?;
                out.push(Indexed::new(idx, read_time_and_interval(cursor)?));
            }
            Ok(handler.on_time_and_intervals(info, &out))
        }
        GroupVariation::Unknown => Err(ParseResult::UnknownObject),
        _ => Err(ParseResult::InvalidObjectQualifier),
    }
}

fn parse_indexed_values(
    cursor: &mut ReadCursor<'_>,
    info: HeaderInfo,
    indices: &[u16],
    cto: Option<DnpTime>,
    handler: &mut dyn ApduHandler,
) -> Result<IinField, ParseResult> {
    match info.gv {
        GroupVariation::Group1Var2
        | GroupVariation::Group2Var1
        | GroupVariation::Group2Var2
        | GroupVariation::Group2Var3 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(idx, read_binary(cursor, info.gv, cto)?));
            }
            Ok(handler.on_binaries(info, &out))
        }
        GroupVariation::Group3Var2
        | GroupVariation::Group4Var1
        | GroupVariation::Group4Var2
        | GroupVariation::Group4Var3 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(idx, read_double_bit(cursor, info.gv, cto)?));
            }
            Ok(handler.on_double_bits(info, &out))
        }
        GroupVariation::Group10Var2 | GroupVariation::Group11Var1 | GroupVariation::Group11Var2 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(
                    idx,
                    read_binary_output_status(cursor, info.gv)?,
                ));
            }
            Ok(handler.on_binary_output_statuses(info, &out))
        }
        GroupVariation::Group20Var1
        | GroupVariation::Group20Var2
        | GroupVariation::Group20Var5
        | GroupVariation::Group20Var6
        | GroupVariation::Group22Var1
        | GroupVariation::Group22Var2
        | GroupVariation::Group22Var5
        | GroupVariation::Group22Var6 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(idx, read_counter(cursor, info.gv)?));
            }
            Ok(handler.on_counters(info, &out))
        }
        GroupVariation::Group21Var1
        | GroupVariation::Group21Var2
        | GroupVariation::Group21Var5
        | GroupVariation::Group21Var6
        | GroupVariation::Group21Var9
        | GroupVariation::Group21Var10
        | GroupVariation::Group23Var1
        | GroupVariation::Group23Var2
        | GroupVariation::Group23Var5
        | GroupVariation::Group23Var6 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(idx, read_frozen_counter(cursor, info.gv)?));
            }
            Ok(handler.on_frozen_counters(info, &out))
        }
        GroupVariation::Group30Var1
        | GroupVariation::Group30Var2
        | GroupVariation::Group30Var3
        | GroupVariation::Group30Var4
        | GroupVariation::Group30Var5
        | GroupVariation::Group30Var6
        | GroupVariation::Group32Var1
        | GroupVariation::Group32Var2
        | GroupVariation::Group32Var3
        | GroupVariation::Group32Var4
        | GroupVariation::Group32Var5
        | GroupVariation::Group32Var6
        | GroupVariation::Group32Var7
        | GroupVariation::Group32Var8 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(idx, read_analog(cursor, info.gv)?));
            }
            Ok(handler.on_analogs(info, &out))
        }
        GroupVariation::Group40Var1
        | GroupVariation::Group40Var2
        | GroupVariation::Group40Var3
        | GroupVariation::Group40Var4
        | GroupVariation::Group42Var1
        | GroupVariation::Group42Var2
        | GroupVariation::Group42Var3
        | GroupVariation::Group42Var4
        | GroupVariation::Group42Var5
        | GroupVariation::Group42Var6
        | GroupVariation::Group42Var7
        | GroupVariation::Group42Var8 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(
                    idx,
                    read_analog_output_status(cursor, info.gv)?,
                ));
            }
            Ok(handler.on_analog_output_statuses(info, &out))
        }
        GroupVariation::Group50Var4 => {
            let mut out = Vec::with_capacity(indices.len());
            for &idx in indices {
                out.push(Indexed::new(idx, read_time_and_interval(cursor)?));
            }
            Ok(handler.on_time_and_intervals(info, &out))
        }
        _ => Err(ParseResult::UnknownObject),
    }
}

fn read_index(cursor: &mut ReadCursor<'_>, index_u16: bool) -> Result<u16, ParseResult> {
    if index_u16 {
        cursor
            .read_u16_le()
            .map_err(|_| ParseResult::NotEnoughDataForObjects)
    } else {
        cursor
            .read_u8()
            .map(u16::from)
            .map_err(|_| ParseResult::NotEnoughDataForObjects)
    }
}

fn read_binary(
    cursor: &mut ReadCursor<'_>,
    gv: GroupVariation,
    cto: Option<DnpTime>,
) -> Result<Binary, ParseResult> {
    let flags = Flags::new(
        cursor
            .read_u8()
            .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
    );
    match gv {
        GroupVariation::Group1Var2 | GroupVariation::Group2Var1 => Ok(Binary::with_flags(flags)),
        GroupVariation::Group2Var2 => {
            let t = read_time48(cursor)?;
            Ok(Binary::with_flags_and_time(flags, t))
        }
        GroupVariation::Group2Var3 => {
            let rel = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            Ok(Binary::with_flags_and_time(
                flags,
                resolve_cto_time(cto, rel),
            ))
        }
        _ => Err(ParseResult::UnknownObject),
    }
}

fn read_double_bit(
    cursor: &mut ReadCursor<'_>,
    gv: GroupVariation,
    cto: Option<DnpTime>,
) -> Result<DoubleBitBinary, ParseResult> {
    let flags = Flags::new(
        cursor
            .read_u8()
            .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
    );
    match gv {
        GroupVariation::Group3Var2 | GroupVariation::Group4Var1 => {
            Ok(DoubleBitBinary::with_flags(flags))
        }
        GroupVariation::Group4Var2 => {
            let t = read_time48(cursor)?;
            Ok(DoubleBitBinary::with_flags_and_time(flags, t))
        }
        GroupVariation::Group4Var3 => {
            let rel = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            Ok(DoubleBitBinary::with_flags_and_time(
                flags,
                resolve_cto_time(cto, rel),
            ))
        }
        _ => Err(ParseResult::UnknownObject),
    }
}

fn read_binary_output_status(
    cursor: &mut ReadCursor<'_>,
    gv: GroupVariation,
) -> Result<BinaryOutputStatus, ParseResult> {
    let flags = Flags::new(
        cursor
            .read_u8()
            .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
    );
    match gv {
        GroupVariation::Group10Var2 | GroupVariation::Group11Var1 => {
            Ok(BinaryOutputStatus::with_flags(flags))
        }
        GroupVariation::Group11Var2 => {
            let t = read_time48(cursor)?;
            Ok(BinaryOutputStatus::with_flags_and_time(flags, t))
        }
        _ => Err(ParseResult::UnknownObject),
    }
}

fn read_counter(cursor: &mut ReadCursor<'_>, gv: GroupVariation) -> Result<Counter, ParseResult> {
    match gv {
        GroupVariation::Group20Var1 | GroupVariation::Group22Var1 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            Ok(Counter::with_flags(val, flags))
        }
        GroupVariation::Group20Var2 | GroupVariation::Group22Var2 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as u32;
            Ok(Counter::with_flags(val, flags))
        }
        GroupVariation::Group20Var5 => {
            let val = cursor
                .read_u32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            Ok(Counter::new(val))
        }
        GroupVariation::Group20Var6 => {
            let val = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as u32;
            Ok(Counter::new(val))
        }
        GroupVariation::Group22Var5 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            let t = read_time48(cursor)?;
            Ok(Counter::with_flags_and_time(val, flags, t))
        }
        GroupVariation::Group22Var6 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as u32;
            let t = read_time48(cursor)?;
            Ok(Counter::with_flags_and_time(val, flags, t))
        }
        _ => Err(ParseResult::UnknownObject),
    }
}

fn read_frozen_counter(
    cursor: &mut ReadCursor<'_>,
    gv: GroupVariation,
) -> Result<FrozenCounter, ParseResult> {
    match gv {
        GroupVariation::Group21Var1 | GroupVariation::Group23Var1 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            Ok(FrozenCounter::with_flags(val, flags))
        }
        GroupVariation::Group21Var2 | GroupVariation::Group23Var2 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as u32;
            Ok(FrozenCounter::with_flags(val, flags))
        }
        GroupVariation::Group21Var5 | GroupVariation::Group23Var5 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            let t = read_time48(cursor)?;
            Ok(FrozenCounter::with_flags_and_time(val, flags, t))
        }
        GroupVariation::Group21Var6 | GroupVariation::Group23Var6 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as u32;
            let t = read_time48(cursor)?;
            Ok(FrozenCounter::with_flags_and_time(val, flags, t))
        }
        GroupVariation::Group21Var9 => {
            let val = cursor
                .read_u32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            Ok(FrozenCounter::new(val))
        }
        GroupVariation::Group21Var10 => {
            let val = cursor
                .read_u16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as u32;
            Ok(FrozenCounter::new(val))
        }
        _ => Err(ParseResult::UnknownObject),
    }
}

fn read_analog(cursor: &mut ReadCursor<'_>, gv: GroupVariation) -> Result<Analog, ParseResult> {
    match gv {
        GroupVariation::Group30Var1 | GroupVariation::Group32Var1 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_i32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            Ok(Analog::with_flags(val, flags))
        }
        GroupVariation::Group30Var2 | GroupVariation::Group32Var2 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_i16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            Ok(Analog::with_flags(val, flags))
        }
        GroupVariation::Group30Var3 => {
            let val = cursor
                .read_i32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            Ok(Analog::new(val))
        }
        GroupVariation::Group30Var4 => {
            let val = cursor
                .read_i16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            Ok(Analog::new(val))
        }
        GroupVariation::Group30Var5 | GroupVariation::Group32Var5 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_f32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            Ok(Analog::with_flags(val, flags))
        }
        GroupVariation::Group30Var6 | GroupVariation::Group32Var6 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_f64_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            Ok(Analog::with_flags(val, flags))
        }
        GroupVariation::Group32Var3 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_i32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            let t = read_time48(cursor)?;
            Ok(Analog::with_flags_and_time(val, flags, t))
        }
        GroupVariation::Group32Var4 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_i16_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            let t = read_time48(cursor)?;
            Ok(Analog::with_flags_and_time(val, flags, t))
        }
        GroupVariation::Group32Var7 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_f32_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)? as f64;
            let t = read_time48(cursor)?;
            Ok(Analog::with_flags_and_time(val, flags, t))
        }
        GroupVariation::Group32Var8 => {
            let flags = Flags::new(
                cursor
                    .read_u8()
                    .map_err(|_| ParseResult::NotEnoughDataForObjects)?,
            );
            let val = cursor
                .read_f64_le()
                .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
            let t = read_time48(cursor)?;
            Ok(Analog::with_flags_and_time(val, flags, t))
        }
        _ => Err(ParseResult::UnknownObject),
    }
}

fn read_analog_output_status(
    cursor: &mut ReadCursor<'_>,
    gv: GroupVariation,
) -> Result<AnalogOutputStatus, ParseResult> {
    let a = match gv {
        GroupVariation::Group40Var1 => read_analog(cursor, GroupVariation::Group30Var1)?,
        GroupVariation::Group40Var2 => read_analog(cursor, GroupVariation::Group30Var2)?,
        GroupVariation::Group40Var3 => read_analog(cursor, GroupVariation::Group30Var5)?,
        GroupVariation::Group40Var4 => read_analog(cursor, GroupVariation::Group30Var6)?,
        GroupVariation::Group42Var1 => read_analog(cursor, GroupVariation::Group32Var1)?,
        GroupVariation::Group42Var2 => read_analog(cursor, GroupVariation::Group32Var2)?,
        GroupVariation::Group42Var3 => read_analog(cursor, GroupVariation::Group32Var3)?,
        GroupVariation::Group42Var4 => read_analog(cursor, GroupVariation::Group32Var4)?,
        GroupVariation::Group42Var5 => read_analog(cursor, GroupVariation::Group32Var5)?,
        GroupVariation::Group42Var6 => read_analog(cursor, GroupVariation::Group32Var6)?,
        GroupVariation::Group42Var7 => read_analog(cursor, GroupVariation::Group32Var7)?,
        GroupVariation::Group42Var8 => read_analog(cursor, GroupVariation::Group32Var8)?,
        _ => return Err(ParseResult::UnknownObject),
    };
    Ok(AnalogOutputStatus::with_flags_and_time(
        a.value, a.flags, a.time,
    ))
}

fn read_time_and_interval(cursor: &mut ReadCursor<'_>) -> Result<TimeAndInterval, ParseResult> {
    let time = read_time48(cursor)?;
    let interval = cursor
        .read_u32_le()
        .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
    let units = cursor
        .read_u8()
        .map_err(|_| ParseResult::NotEnoughDataForObjects)?;
    Ok(TimeAndInterval {
        time,
        interval,
        units,
    })
}
