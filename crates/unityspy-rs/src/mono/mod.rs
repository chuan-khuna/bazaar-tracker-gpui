//! The Mono runtime's metadata, read through [`crate::mem::reader::Memory`].

pub mod bcl;
pub mod class;
pub mod domain;
pub mod field;
pub mod object;
pub mod offsets;
pub mod pe;
pub mod statics;
pub mod value;

#[cfg(test)]
pub(crate) mod testing;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{Error, Result};
use crate::mem::module::{self, Module};
use crate::mem::process::{self, FileVersion, Process};
use crate::mem::reader::Memory;
use class::Class;
use domain::Assembly;
use offsets::{MonoOffsets, UnityVersion};

/// The name of Unity's Mono runtime module.
pub const MONO_MODULE: &str = "mono-2.0-bdwgc.dll";
/// The export that returns the root domain.
pub const ROOT_DOMAIN_EXPORT: &str = "mono_get_root_domain";

/// What attaching found out, for messages.
#[derive(Clone, Debug)]
pub struct AttachInfo {
    pub pid: u32,
    pub mono_module: Module,
    pub file_version: FileVersion,
    pub unity_version: UnityVersion,
}

/// A Mono runtime in some address space. Every read goes through the [`Memory`] `M`.
pub struct Mono<M: Memory> {
    memory: M,
    offsets: &'static MonoOffsets,
    domain: u64,
    caches: Mutex<Caches>,
}

impl<M: Memory> Mono<M> {
    /// A runtime whose root domain is already known. Tests use this with byte buffers.
    pub fn new(memory: M, offsets: &'static MonoOffsets, domain: u64) -> Self {
        Self { memory, offsets, domain, caches: Mutex::default() }
    }

    pub fn memory(&self) -> &M {
        &self.memory
    }

    pub fn offsets(&self) -> &'static MonoOffsets {
        self.offsets
    }

    pub fn root_domain(&self) -> u64 {
        self.domain
    }

    /// Every assembly loaded in the root domain.
    pub fn assemblies(&self) -> Result<Vec<Assembly>> {
        domain::assemblies(&self.memory, self.offsets, self.domain)
    }

    /// The assembly called `name`, or an error naming it.
    pub fn assembly(&self, name: &str) -> Result<Assembly> {
        domain::find_assembly(&self.memory, self.offsets, self.domain, name)
    }
}

impl Mono<Process> {
    /// Attach to a game already opened as `process`: find the Mono module and its root domain, and
    /// choose the offsets table from the game executable's version.
    pub fn attach(process: Process) -> Result<(Self, AttachInfo)> {
        let image_path = process.image_path()?;
        let file_version = process::file_version(&image_path).ok_or_else(|| Error::UnsupportedUnityVersion(format!("(no version resource in {})", image_path.display())))?;
        let unity_version = UnityVersion::new(file_version.major, file_version.minor, file_version.patch);
        let offsets = offsets::for_version(unity_version)?;
        let mono_module = module::find_module(&process, MONO_MODULE)?;
        let function = pe::find_export(&process, mono_module.base, ROOT_DOMAIN_EXPORT)?;
        let domain = domain::root_domain(&process, function)?;
        let info = AttachInfo { pid: process.pid(), mono_module, file_version, unity_version };
        Ok((Self::new(process, offsets, domain), info))
    }
}

/// What has been learned about classes, kept for as long as the runtime is attached. Dropped with
/// the `Mono`, so a game restart (a new attach) starts clean.
#[derive(Default)]
struct Caches {
    by_address: HashMap<u64, Arc<Class>>,
    /// `(image, full name)` to the class's address.
    by_name: HashMap<(u64, String), u64>,
}
