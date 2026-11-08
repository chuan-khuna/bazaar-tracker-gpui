//! Finding a loaded module (such as `mono-2.0-bdwgc.dll` or `libmonobdwgc-2.0.dylib`, matched
//! case-insensitively) in a process. The listing itself is in `module_windows.rs` or `module_macos.rs`.

use std::path::{Path, PathBuf};

use super::process::Process;
use crate::error::{Error, Result};

#[cfg(target_os = "macos")]
#[path = "module_macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "module_windows.rs"]
mod platform;

pub use platform::modules;

/// A module (an exe, DLL or dylib) loaded in a process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    /// The file name, such as `mono-2.0-bdwgc.dll`.
    pub name: String,
    pub path: PathBuf,
    pub base: u64,
    /// The size of the image in memory. 0 on macOS, where dyld's list does not carry it.
    pub size: u64,
}

/// Whether the module at `path` is called `wanted`, ignoring case.
pub fn name_matches(path: &Path, wanted: &str) -> bool {
    path.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(wanted))
}

/// The module of `process` called `name` (ignoring case), or an error naming it.
pub fn find_module(process: &Process, name: &str) -> Result<Module> {
    modules(process)?.into_iter().find(|module| name_matches(&module.path, name)).ok_or_else(|| Error::ModuleNotFound(name.to_owned()))
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
