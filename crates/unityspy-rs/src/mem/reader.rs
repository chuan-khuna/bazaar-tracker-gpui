//! The one door every Mono structure is read through, so tests can use byte buffers instead of a live game.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{Error, Result};

/// Reads raw bytes from some address space: a live process, or a buffer in a test.
pub trait Memory {
    /// Fill `buffer` with the bytes at `address`, or say why that failed. A read that can only be
    /// partly satisfied is a failure.
    fn read(&self, address: u64, buffer: &mut [u8]) -> std::io::Result<()>;
}

impl<M: Memory + ?Sized> Memory for &M {
    fn read(&self, address: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        (**self).read(address, buffer)
    }
}

/// The longest C string (a class, field or assembly name) we will read.
const MAX_C_STRING: usize = 1024;

/// Typed reads, built on [`Memory::read`]. Every Mono structure on a 64-bit game is read with these.
pub trait MemoryExt: Memory {
    fn read_bytes(&self, address: u64, len: usize) -> Result<Vec<u8>> {
        let mut bytes = vec![0; len];
        self.read_into(address, &mut bytes)?;
        Ok(bytes)
    }

    fn read_into(&self, address: u64, buffer: &mut [u8]) -> Result<()> {
        self.read(address, buffer).map_err(|source| Error::Read { address, len: buffer.len(), source })
    }

    fn read_u8(&self, address: u64) -> Result<u8> {
        let mut bytes = [0; 1];
        self.read_into(address, &mut bytes)?;
        Ok(bytes[0])
    }

    fn read_i8(&self, address: u64) -> Result<i8> {
        Ok(self.read_u8(address)? as i8)
    }

    fn read_u16(&self, address: u64) -> Result<u16> {
        let mut bytes = [0; 2];
        self.read_into(address, &mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_i16(&self, address: u64) -> Result<i16> {
        Ok(self.read_u16(address)? as i16)
    }

    fn read_u32(&self, address: u64) -> Result<u32> {
        let mut bytes = [0; 4];
        self.read_into(address, &mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_i32(&self, address: u64) -> Result<i32> {
        Ok(self.read_u32(address)? as i32)
    }

    fn read_u64(&self, address: u64) -> Result<u64> {
        let mut bytes = [0; 8];
        self.read_into(address, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_i64(&self, address: u64) -> Result<i64> {
        Ok(self.read_u64(address)? as i64)
    }

    /// A 32-bit float: four bytes (UnitySpy read two).
    fn read_f32(&self, address: u64) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32(address)?))
    }

    /// A 64-bit float: eight bytes (UnitySpy read two).
    fn read_f64(&self, address: u64) -> Result<f64> {
        Ok(f64::from_bits(self.read_u64(address)?))
    }

    /// A pointer in the 64-bit game.
    fn read_ptr(&self, address: u64) -> Result<u64> {
        self.read_u64(address)
    }

    /// A NUL-terminated ASCII (UTF-8) string, such as a class name. It is read in chunks that never
    /// cross a 4 KiB page boundary, so a string at the end of a mapped region does not fail the way
    /// UnitySpy's fixed 1,024-byte read did.
    fn read_c_string(&self, address: u64) -> Result<String> {
        let mut bytes = Vec::new();
        let mut at = address;
        while bytes.len() < MAX_C_STRING {
            let to_page_end = 4096 - (at % 4096) as usize;
            let chunk_len = to_page_end.min(MAX_C_STRING - bytes.len()).min(256);
            let chunk = self.read_bytes(at, chunk_len)?;
            if let Some(end) = chunk.iter().position(|&byte| byte == 0) {
                bytes.extend_from_slice(&chunk[..end]);
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }
            bytes.extend_from_slice(&chunk);
            at += chunk_len as u64;
        }
        Err(Error::Implausible { what: "C string length", value: bytes.len() as i64 })
    }
}

impl<M: Memory + ?Sized> MemoryExt for M {}

const PAGE: u64 = 4096;

/// A fake address space made of byte buffers, for tests. Memory is mapped a page at a time as it is
/// written; unwritten bytes in a mapped page read as zero and reading outside a mapped page fails,
/// as it does in a real process.
#[derive(Default)]
pub struct BufferMemory {
    pages: Mutex<HashMap<u64, Box<[u8; PAGE as usize]>>>,
}

impl BufferMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Write `bytes` at `address`, mapping the pages they touch.
    pub fn write(&self, address: u64, bytes: &[u8]) {
        let mut pages = self.pages.lock().unwrap();
        for (i, byte) in bytes.iter().enumerate() {
            let at = address + i as u64;
            pages.entry(at / PAGE).or_insert_with(|| Box::new([0; PAGE as usize]))[(at % PAGE) as usize] = *byte;
        }
    }

    /// Map the pages of `address..address + len` without writing anything.
    pub fn map(&self, address: u64, len: u64) {
        let mut pages = self.pages.lock().unwrap();
        for page in address / PAGE..=(address + len.max(1) - 1) / PAGE {
            pages.entry(page).or_insert_with(|| Box::new([0; PAGE as usize]));
        }
    }

    pub fn write_u8(&self, address: u64, value: u8) {
        self.write(address, &[value]);
    }

    pub fn write_u16(&self, address: u64, value: u16) {
        self.write(address, &value.to_le_bytes());
    }

    pub fn write_u32(&self, address: u64, value: u32) {
        self.write(address, &value.to_le_bytes());
    }

    pub fn write_i32(&self, address: u64, value: i32) {
        self.write(address, &value.to_le_bytes());
    }

    pub fn write_u64(&self, address: u64, value: u64) {
        self.write(address, &value.to_le_bytes());
    }

    pub fn write_f32(&self, address: u64, value: f32) {
        self.write(address, &value.to_le_bytes());
    }

    pub fn write_f64(&self, address: u64, value: f64) {
        self.write(address, &value.to_le_bytes());
    }

    /// Write `text` and a NUL after it.
    pub fn write_c_string(&self, address: u64, text: &str) {
        self.write(address, text.as_bytes());
        self.write_u8(address + text.len() as u64, 0);
    }
}

impl Memory for BufferMemory {
    fn read(&self, address: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        let pages = self.pages.lock().unwrap();
        for (i, slot) in buffer.iter_mut().enumerate() {
            let at = address.wrapping_add(i as u64);
            let Some(page) = pages.get(&(at / PAGE)) else {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, format!("address {at:#x} is not mapped")));
            };
            *slot = page[(at % PAGE) as usize];
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "reader_tests.rs"]
mod tests;
