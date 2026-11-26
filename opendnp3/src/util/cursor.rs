//! Bounds-checked little-endian read/write cursors over byte slices.
//!
//! These are the Rust equivalent of ser4cpp's `rseq_t`/`wseq_t` used
//! throughout the original C++ implementation.

use crate::error::{ParseError, WriteError};

/// A cursor that reads little-endian primitives from a byte slice.
#[derive(Clone, Debug)]
pub struct ReadCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ReadCursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Number of bytes not yet consumed.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Number of bytes consumed so far.
    pub fn consumed(&self) -> usize {
        self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// All remaining bytes, without consuming them.
    pub fn peek(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    /// Rewind to the beginning.
    pub fn reset(&mut self) {
        self.pos = 0;
    }

    /// Skip `n` bytes.
    pub fn advance(&mut self, n: usize) -> Result<(), ParseError> {
        if self.remaining() < n {
            return Err(ParseError::InsufficientBytes);
        }
        self.pos += n;
        Ok(())
    }

    pub fn read_u8(&mut self) -> Result<u8, ParseError> {
        if self.remaining() < 1 {
            return Err(ParseError::InsufficientBytes);
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    pub fn read_u16_le(&mut self) -> Result<u16, ParseError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_u32_le(&mut self) -> Result<u32, ParseError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Read a 48-bit unsigned little-endian integer (DNP3 timestamps).
    pub fn read_u48_le(&mut self) -> Result<u64, ParseError> {
        let b = self.read_bytes(6)?;
        let mut buf = [0u8; 8];
        buf[..6].copy_from_slice(b);
        Ok(u64::from_le_bytes(buf))
    }

    pub fn read_i16_le(&mut self) -> Result<i16, ParseError> {
        Ok(self.read_u16_le()? as i16)
    }

    pub fn read_i32_le(&mut self) -> Result<i32, ParseError> {
        Ok(self.read_u32_le()? as i32)
    }

    pub fn read_f32_le(&mut self) -> Result<f32, ParseError> {
        Ok(f32::from_bits(self.read_u32_le()?))
    }

    pub fn read_f64_le(&mut self) -> Result<f64, ParseError> {
        let b = self.read_bytes(8)?;
        Ok(f64::from_le_bytes(b.try_into().unwrap()))
    }

    /// Read `n` bytes as a slice.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        if self.remaining() < n {
            return Err(ParseError::InsufficientBytes);
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    /// Read the remaining bytes.
    pub fn read_all(&mut self) -> &'a [u8] {
        let s = self.peek();
        self.pos = self.data.len();
        s
    }
}

/// A cursor that writes little-endian primitives into a byte slice.
pub struct WriteCursor<'a> {
    data: &'a mut [u8],
    pos: usize,
}

impl<'a> WriteCursor<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Remaining writable space.
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Number of bytes written so far.
    pub fn written_len(&self) -> usize {
        self.pos
    }

    pub fn is_full(&self) -> bool {
        self.remaining() == 0
    }

    /// Consume the cursor and return the written prefix.
    pub fn into_written(self) -> &'a [u8] {
        &self.data[..self.pos]
    }

    /// Write a single byte.
    pub fn write_u8(&mut self, v: u8) -> Result<(), WriteError> {
        if self.remaining() < 1 {
            return Err(WriteError::InsufficientSpace);
        }
        self.data[self.pos] = v;
        self.pos += 1;
        Ok(())
    }

    pub fn write_u16_le(&mut self, v: u16) -> Result<(), WriteError> {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_u32_le(&mut self, v: u32) -> Result<(), WriteError> {
        self.write_bytes(&v.to_le_bytes())
    }

    /// Write the low 48 bits of `v` little-endian (DNP3 timestamps).
    pub fn write_u48_le(&mut self, v: u64) -> Result<(), WriteError> {
        let bytes = v.to_le_bytes();
        self.write_bytes(&bytes[..6])
    }

    pub fn write_i16_le(&mut self, v: i16) -> Result<(), WriteError> {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_i32_le(&mut self, v: i32) -> Result<(), WriteError> {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_f32_le(&mut self, v: f32) -> Result<(), WriteError> {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_f64_le(&mut self, v: f64) -> Result<(), WriteError> {
        self.write_bytes(&v.to_le_bytes())
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), WriteError> {
        if self.remaining() < bytes.len() {
            return Err(WriteError::InsufficientSpace);
        }
        self.data[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }

    /// Borrow the written prefix without consuming the cursor.
    pub fn written(&self) -> &[u8] {
        &self.data[..self.pos]
    }

    /// Mutable access to the not-yet-written remainder.
    pub fn unwritten(&mut self) -> &mut [u8] {
        &mut self.data[self.pos..]
    }

    /// Move the position forward by `n` bytes (e.g. after writing through
    /// [`unwritten`](Self::unwritten) directly).
    pub fn advance(&mut self, n: usize) -> Result<(), WriteError> {
        if self.remaining() < n {
            return Err(WriteError::InsufficientSpace);
        }
        self.pos += n;
        Ok(())
    }
}
