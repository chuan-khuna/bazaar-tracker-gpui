//! The Mono root domain and the assemblies loaded in it.

use super::offsets::MonoOffsets;
use crate::error::{Error, Result};
use crate::mem::reader::{Memory, MemoryExt};

/// The most assemblies we will walk before deciding the list is corrupt.
const MAX_ASSEMBLIES: usize = 4096;

/// An assembly loaded in the domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assembly {
    /// The simple name, with no extension: `TheBazaarRuntime`, `mscorlib`.
    pub name: String,
    /// The address of its `MonoAssembly`.
    pub address: u64,
    /// The address of its `MonoImage`.
    pub image: u64,
}

/// Read the root domain from the body of `mono_get_root_domain`, which loads one global. The bytes
/// are checked before any displacement is trusted, because a different compiler or build would give
/// a garbage pointer. The forms known are:
///
/// - Windows x64: `mov rax,[rip+rel32]; ret` (`48 8B 05 <rel32> C3`).
/// - macOS x64: `push rbp; mov rbp,rsp; mov rax,[rip+rel32]; pop rbp; ret`
///   (`55 48 89 E5 48 8B 05 <rel32> 5D C3`).
/// - macOS arm64: `adrp xN, page; ldr x0, [xN, #imm]; ret`.
pub fn root_domain(memory: &impl Memory, function: u64) -> Result<u64> {
    let mut code = [0u8; 16];
    memory.read_into(function, &mut code)?;
    let global = global_loaded_by(function, &code).ok_or_else(|| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&code[..8]);
        Error::UnexpectedRootDomainCode { address: function, bytes }
    })?;
    match memory.read_ptr(global)? {
        0 => Err(Error::RootDomainNull),
        domain => Ok(domain),
    }
}

/// The address of the global that the function at `function`, whose first bytes are `code`, returns.
fn global_loaded_by(function: u64, code: &[u8; 16]) -> Option<u64> {
    let rip_relative = |start: usize| {
        let rel32 = i32::from_le_bytes(code[start + 3..start + 7].try_into().expect("4 bytes"));
        // The displacement is relative to the end of the 7-byte `mov`.
        function.wrapping_add(start as u64 + 7).wrapping_add_signed(i64::from(rel32))
    };
    if code[..3] == [0x48, 0x8B, 0x05] && code[7] == 0xC3 {
        return Some(rip_relative(0));
    }
    if code[..7] == [0x55, 0x48, 0x89, 0xE5, 0x48, 0x8B, 0x05] && code[11..13] == [0x5D, 0xC3] {
        return Some(rip_relative(4));
    }
    arm64_adrp_ldr_ret(function, code)
}

/// `adrp xN, page; ldr x0, [xN, #imm]; ret`: the global is at `page + imm`.
fn arm64_adrp_ldr_ret(function: u64, code: &[u8; 16]) -> Option<u64> {
    let instruction = |index: usize| u32::from_le_bytes(code[index * 4..index * 4 + 4].try_into().expect("4 bytes"));
    let (adrp, ldr, ret) = (instruction(0), instruction(1), instruction(2));
    const RET: u32 = 0xD65F_03C0;
    // ADRP: op=1, bits 28..24 = 10000. LDR (immediate, unsigned offset, 64-bit): 0xF94 prefix.
    if adrp & 0x9F00_0000 != 0x9000_0000 || ldr & 0xFFC0_0000 != 0xF940_0000 || ret != RET {
        return None;
    }
    let adrp_register = adrp & 0x1F;
    let (ldr_target, ldr_base) = (ldr & 0x1F, (ldr >> 5) & 0x1F);
    if ldr_target != 0 || ldr_base != adrp_register {
        return None;
    }
    let immediate = (((adrp >> 5) & 0x7_FFFF) << 2) | ((adrp >> 29) & 0x3);
    // Sign-extend the 21-bit page count, then make it a byte offset.
    let pages = (i64::from(immediate) << 43) >> 43;
    let page = (function & !0xFFF).wrapping_add_signed(pages << 12);
    let offset = u64::from((ldr >> 10) & 0xFFF) * 8;
    Some(page.wrapping_add(offset))
}

/// Every assembly loaded in `domain`: a walk of the `GSList` of `MonoAssembly`s.
pub fn assemblies(memory: &impl Memory, offsets: &MonoOffsets, domain: u64) -> Result<Vec<Assembly>> {
    let mut found = Vec::new();
    let mut node = memory.read_ptr(domain + offsets.domain_assemblies)?;
    while node != 0 {
        if found.len() >= MAX_ASSEMBLIES {
            return Err(Error::Loop("assembly list"));
        }
        let address = memory.read_ptr(node)?;
        // `MonoAssembly` starts with an int ref count (padded) and `basedir`; `aname.name` follows.
        let name = memory.read_c_string(memory.read_ptr(address + 16)?)?;
        let image = memory.read_ptr(address + offsets.assembly_image)?;
        found.push(Assembly { name, address, image });
        node = memory.read_ptr(node + 8)?;
    }
    Ok(found)
}

/// The assembly called `name` (exactly), or an error naming it.
pub fn find_assembly(memory: &impl Memory, offsets: &MonoOffsets, domain: u64, name: &str) -> Result<Assembly> {
    assemblies(memory, offsets, domain)?.into_iter().find(|assembly| assembly.name == name).ok_or_else(|| Error::AssemblyNotFound(name.to_owned()))
}

#[cfg(test)]
#[path = "domain_tests.rs"]
mod tests;
