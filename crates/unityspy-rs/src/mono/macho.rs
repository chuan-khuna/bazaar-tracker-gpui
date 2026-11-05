//! Looking up an exported function in a 64-bit Mach-O image (a dylib) loaded in the target process,
//! the macOS counterpart of [`super::pe`]. It reads the header and load commands at the load
//! address, then the symbol and string tables from `__LINKEDIT`, all from the process's memory, so
//! the slice of a universal file that is actually running is the one read.

use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};

const MH_MAGIC_64: u32 = 0xFEED_FACF;
const MACH_HEADER_64_SIZE: u64 = 32;
const LC_SYMTAB: u32 = 0x2;
const LC_SEGMENT_64: u32 = 0x19;
const NLIST_64_SIZE: usize = 16;
/// `n_type` bits: a debugging (stab) entry, and the type field.
const N_STAB: u8 = 0xE0;
const N_TYPE: u8 = 0x0E;
/// Defined in a section of this image.
const N_SECT: u8 = 0x0E;
const MAX_LOAD_COMMANDS: u32 = 4096;
const MAX_SYMBOLS: u32 = 4_000_000;
const MAX_STRING_TABLE: u32 = 256 << 20;

struct Segment {
    vmaddr: u64,
    fileoff: u64,
}

/// The address of the function `name` (a C name, without the leading `_` Mach-O adds) in the image
/// loaded at `base`.
pub fn find_export(memory: &impl Memory, base: u64, name: &str) -> Result<u64> {
    if memory.read_u32(base)? != MH_MAGIC_64 {
        return Err(Error::BadMachO("no 64-bit Mach-O magic"));
    }
    let commands = memory.read_u32(base + 16)?;
    let commands_size = memory.read_u32(base + 20)?;
    if commands > MAX_LOAD_COMMANDS {
        return Err(Error::BadMachO("implausible load command count"));
    }
    let table = memory.read_bytes(base + MACH_HEADER_64_SIZE, commands_size as usize)?;
    let (mut text, mut linkedit, mut symtab) = (None, None, None);
    let mut at = 0usize;
    for _ in 0..commands {
        let command = table.get(at..at + 8).ok_or(Error::BadMachO("load commands run past their size"))?;
        let kind = u32::from_le_bytes(command[0..4].try_into().expect("4 bytes"));
        let size = u32::from_le_bytes(command[4..8].try_into().expect("4 bytes")) as usize;
        let body = table.get(at..at + size).filter(|_| size >= 8).ok_or(Error::BadMachO("a load command runs past the table"))?;
        let u32_at = |offset: usize| body.get(offset..offset + 4).map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4 bytes")));
        let u64_at = |offset: usize| body.get(offset..offset + 8).map(|bytes| u64::from_le_bytes(bytes.try_into().expect("8 bytes")));
        match kind {
            LC_SEGMENT_64 => {
                let segment_name = body.get(8..24).ok_or(Error::BadMachO("short segment command"))?;
                let segment_name = &segment_name[..segment_name.iter().position(|&byte| byte == 0).unwrap_or(16)];
                let segment = Segment { vmaddr: u64_at(24).ok_or(Error::BadMachO("short segment command"))?, fileoff: u64_at(40).ok_or(Error::BadMachO("short segment command"))? };
                match segment_name {
                    b"__TEXT" => text = Some(segment),
                    b"__LINKEDIT" => linkedit = Some(segment),
                    _ => {}
                }
            }
            LC_SYMTAB => {
                let fields = (u32_at(8), u32_at(12), u32_at(16), u32_at(20));
                let (Some(symoff), Some(nsyms), Some(stroff), Some(strsize)) = fields else {
                    return Err(Error::BadMachO("short symtab command"));
                };
                symtab = Some((symoff, nsyms, stroff, strsize));
            }
            _ => {}
        }
        at += size;
    }
    let text = text.ok_or(Error::BadMachO("no __TEXT segment"))?;
    let linkedit = linkedit.ok_or(Error::BadMachO("no __LINKEDIT segment"))?;
    let (symoff, nsyms, stroff, strsize) = symtab.ok_or(Error::BadMachO("no symbol table"))?;
    if nsyms > MAX_SYMBOLS || strsize > MAX_STRING_TABLE {
        return Err(Error::BadMachO("implausible symbol table size"));
    }
    // How far the image was moved from the addresses it was linked at, and where the file offsets
    // of `__LINKEDIT` landed in memory.
    let slide = base.wrapping_sub(text.vmaddr);
    let linkedit_at = |file_offset: u32| linkedit.vmaddr.wrapping_add(slide).wrapping_add(u64::from(file_offset)).wrapping_sub(linkedit.fileoff);
    let symbols = memory.read_bytes(linkedit_at(symoff), nsyms as usize * NLIST_64_SIZE)?;
    let strings = memory.read_bytes(linkedit_at(stroff), strsize as usize)?;
    let wanted = format!("_{name}");
    for symbol in symbols.as_chunks::<NLIST_64_SIZE>().0 {
        let kind = symbol[4];
        if kind & N_STAB != 0 || kind & N_TYPE != N_SECT {
            continue;
        }
        let string = u32::from_le_bytes(symbol[0..4].try_into().expect("4 bytes")) as usize;
        let Some(rest) = strings.get(string..) else { continue };
        let symbol_name = &rest[..rest.iter().position(|&byte| byte == 0).unwrap_or(rest.len())];
        if symbol_name == wanted.as_bytes() {
            let value = u64::from_le_bytes(symbol[8..16].try_into().expect("8 bytes"));
            return Ok(value.wrapping_add(slide));
        }
    }
    Err(Error::ExportNotFound(name.to_owned()))
}

#[cfg(test)]
#[path = "macho_tests.rs"]
mod tests;
