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

/// Read the root domain from the body of `mono_get_root_domain`, which is
/// `mov rax,[rip+rel32]; ret` (`48 8B 05 <rel32> C3`). The bytes are checked before the `rel32` is
/// trusted, because a different compiler or build would give a garbage pointer.
pub fn root_domain(memory: &impl Memory, function: u64) -> Result<u64> {
    let mut code = [0u8; 8];
    memory.read_into(function, &mut code)?;
    if code[..3] != [0x48, 0x8B, 0x05] || code[7] != 0xC3 {
        return Err(Error::UnexpectedRootDomainCode { address: function, bytes: code });
    }
    let rel32 = i32::from_le_bytes([code[3], code[4], code[5], code[6]]);
    // The displacement is relative to the end of the 7-byte `mov`.
    let global = function.wrapping_add(7).wrapping_add_signed(i64::from(rel32));
    match memory.read_ptr(global)? {
        0 => Err(Error::RootDomainNull),
        domain => Ok(domain),
    }
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
