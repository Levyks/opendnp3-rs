//! Master command set builder (CROB and Analog Output commands).

use alloc::vec;
use alloc::vec::Vec;

use crate::app::{
    AnalogOutputDouble64, AnalogOutputFloat32, AnalogOutputInt16, AnalogOutputInt32, ApduWriter,
    ControlRelayOutputBlock, Indexed,
};
use crate::enums::QualifierCode;

/// Controls whether command headers may use 1-byte count/index qualifiers (`0x17`)
/// when all indices and the count fit in `u8`, or always use 2-byte (`0x28`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexQualifierMode {
    #[default]
    AlwaysTwoBytes,
    AllowOneByte,
}

/// A typed command header inside a [`CommandSet`].
#[derive(Debug, Clone)]
pub enum CommandHeader {
    Crob(Vec<Indexed<ControlRelayOutputBlock>>),
    AnalogOutputInt32(Vec<Indexed<AnalogOutputInt32>>),
    AnalogOutputInt16(Vec<Indexed<AnalogOutputInt16>>),
    AnalogOutputFloat32(Vec<Indexed<AnalogOutputFloat32>>),
    AnalogOutputDouble64(Vec<Indexed<AnalogOutputDouble64>>),
}

/// A collection of control commands to send in a single SELECT/OPERATE/DIRECT_OPERATE request.
#[derive(Debug, Clone, Default)]
pub struct CommandSet {
    pub headers: Vec<CommandHeader>,
}

impl CommandSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_crob(&mut self, index: u16, cmd: ControlRelayOutputBlock) -> &mut Self {
        self.headers
            .push(CommandHeader::Crob(vec![Indexed::new(index, cmd)]));
        self
    }

    pub fn add_ao_i32(&mut self, index: u16, cmd: AnalogOutputInt32) -> &mut Self {
        self.headers
            .push(CommandHeader::AnalogOutputInt32(vec![Indexed::new(
                index, cmd,
            )]));
        self
    }

    pub fn add_ao_i16(&mut self, index: u16, cmd: AnalogOutputInt16) -> &mut Self {
        self.headers
            .push(CommandHeader::AnalogOutputInt16(vec![Indexed::new(
                index, cmd,
            )]));
        self
    }

    pub fn add_ao_f32(&mut self, index: u16, cmd: AnalogOutputFloat32) -> &mut Self {
        self.headers
            .push(CommandHeader::AnalogOutputFloat32(vec![Indexed::new(
                index, cmd,
            )]));
        self
    }

    pub fn add_ao_f64(&mut self, index: u16, cmd: AnalogOutputDouble64) -> &mut Self {
        self.headers
            .push(CommandHeader::AnalogOutputDouble64(vec![Indexed::new(
                index, cmd,
            )]));
        self
    }

    pub fn write(&self, writer: &mut ApduWriter) -> bool {
        self.write_with_mode(writer, IndexQualifierMode::AlwaysTwoBytes)
    }

    pub fn write_with_mode(&self, writer: &mut ApduWriter, mode: IndexQualifierMode) -> bool {
        let mark = writer.mark();
        for hdr in &self.headers {
            let ok = match hdr {
                CommandHeader::Crob(items) => {
                    Self::write_header_items(writer, 12, 1, items, mode, |w, v| w.write_crob(v))
                }
                CommandHeader::AnalogOutputInt32(items) => {
                    Self::write_header_items(writer, 41, 1, items, mode, |w, v| w.write_ao_i32(v))
                }
                CommandHeader::AnalogOutputInt16(items) => {
                    Self::write_header_items(writer, 41, 2, items, mode, |w, v| w.write_ao_i16(v))
                }
                CommandHeader::AnalogOutputFloat32(items) => {
                    Self::write_header_items(writer, 41, 3, items, mode, |w, v| w.write_ao_f32(v))
                }
                CommandHeader::AnalogOutputDouble64(items) => {
                    Self::write_header_items(writer, 41, 4, items, mode, |w, v| w.write_ao_f64(v))
                }
            };
            if !ok {
                writer.rollback(mark);
                return false;
            }
        }
        true
    }

    fn write_header_items<T>(
        writer: &mut ApduWriter,
        group: u8,
        variation: u8,
        items: &[Indexed<T>],
        mode: IndexQualifierMode,
        write_val: impl Fn(&mut ApduWriter, &T) -> bool,
    ) -> bool {
        if items.is_empty() {
            return true;
        }
        let use_one_byte = mode == IndexQualifierMode::AllowOneByte
            && items.len() <= u8::MAX as usize
            && items.iter().all(|it| it.index <= u8::MAX as u16);

        if use_one_byte {
            let q = QualifierCode::Uint8CntUint8Index.to_u8();
            if !writer.write_bytes(&[group, variation, q, items.len() as u8]) {
                return false;
            }
            for it in items {
                if !writer.write_u8(it.index as u8) || !write_val(writer, &it.value) {
                    return false;
                }
            }
        } else {
            let q = QualifierCode::Uint16CntUint16Index.to_u8();
            if !writer.write_bytes(&[group, variation, q]) || !writer.write_u16(items.len() as u16)
            {
                return false;
            }
            for it in items {
                if !writer.write_u16(it.index) || !write_val(writer, &it.value) {
                    return false;
                }
            }
        }
        true
    }
}
