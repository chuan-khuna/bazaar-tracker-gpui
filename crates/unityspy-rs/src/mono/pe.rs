//! Looking up an exported function in a PE module that is loaded in the target process, by reading
//! only its headers and export directory (not the whole image, as UnitySpy did).

use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};

/// The address of the export `name` in the module loaded at `base`. The function is found through
/// `AddressOfNameOrdinals`, so it is right even when a name's position differs from its ordinal.
pub fn find_export(memory: &impl Memory, base: u64, name: &str) -> Result<u64> {
    if memory.read_u16(base)? != 0x5A4D {
        return Err(Error::BadPe("no MZ signature"));
    }
    let nt = base + u64::from(memory.read_u32(base + 0x3C)?);
    if memory.read_u32(nt)? != 0x0000_4550 {
        return Err(Error::BadPe("no PE signature"));
    }
    // The export directory is the first data directory; the optional header's magic says where it is.
    let directory_offset = match memory.read_u16(nt + 0x18)? {
        0x20B => 0x88,
        0x10B => 0x78,
        _ => return Err(Error::BadPe("unknown optional header")),
    };
    let directory_rva = u64::from(memory.read_u32(nt + directory_offset)?);
    let directory_size = u64::from(memory.read_u32(nt + directory_offset + 4)?);
    if directory_rva == 0 {
        return Err(Error::ExportNotFound(name.to_owned()));
    }
    let directory = base + directory_rva;
    let number_of_names = memory.read_u32(directory + 0x18)? as usize;
    let functions = base + u64::from(memory.read_u32(directory + 0x1C)?);
    let names = base + u64::from(memory.read_u32(directory + 0x20)?);
    let ordinals = base + u64::from(memory.read_u32(directory + 0x24)?);
    if number_of_names > 0x10000 {
        return Err(Error::BadPe("implausible export count"));
    }
    let wanted = name.as_bytes();
    let name_rvas = memory.read_bytes(names, number_of_names * 4)?;
    for (index, rva) in name_rvas.as_chunks::<4>().0.iter().enumerate() {
        let rva = u64::from(u32::from_le_bytes(*rva));
        // The wanted name and its NUL: no need to read any longer name.
        let mut candidate = vec![0u8; wanted.len() + 1];
        if memory.read(base + rva, &mut candidate).is_err() || candidate[..wanted.len()] != *wanted || candidate[wanted.len()] != 0 {
            continue;
        }
        let ordinal = u64::from(memory.read_u16(ordinals + 2 * index as u64)?);
        let function_rva = u64::from(memory.read_u32(functions + 4 * ordinal)?);
        if function_rva >= directory_rva && function_rva < directory_rva + directory_size {
            return Err(Error::BadPe("the export is forwarded to another module"));
        }
        return Ok(base + function_rva);
    }
    Err(Error::ExportNotFound(name.to_owned()))
}

#[cfg(test)]
#[path = "pe_tests.rs"]
mod tests;
