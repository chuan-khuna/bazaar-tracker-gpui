//! Finding a process by name and opening it for reading only.
//!
//! Nothing here can write to the process or change it. The platform part is in `process_windows.rs`
//! (`OpenProcess` with `PROCESS_VM_READ`) or `process_macos.rs` (`task_for_pid`, which needs the
//! reader to be signed with the `com.apple.security.cs.debugger` entitlement). If opening is
//! refused, the error says so; this crate never asks for more rights.

use std::path::Path;

#[cfg(target_os = "macos")]
#[path = "process_macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "process_windows.rs"]
mod platform;

#[cfg(windows)]
pub(crate) use platform::RawHandle;
pub use platform::{Process, file_version};
#[cfg(target_os = "macos")]
pub(crate) use platform::task_port;

/// A file's four-part version (`a.b.c.d`): from the version resource on Windows, from the Unity
/// player's `Info.plist` on macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub build: u16,
}

impl std::fmt::Display for FileVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.major, self.minor, self.patch, self.build)
    }
}

/// Whether the executable file name `exe` is the process name `wanted` (ignoring case and a `.exe`).
fn exe_matches(exe: &str, wanted: &str) -> bool {
    let strip = |name: &str| name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(name).to_lowercase();
    strip(exe) == strip(wanted)
}

/// Whether the executable at `path` is the process name `wanted` (see [`exe_matches`]).
#[cfg_attr(windows, allow(dead_code))]
fn path_matches(path: &Path, wanted: &str) -> bool {
    path.file_name().is_some_and(|name| exe_matches(&name.to_string_lossy(), wanted))
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
