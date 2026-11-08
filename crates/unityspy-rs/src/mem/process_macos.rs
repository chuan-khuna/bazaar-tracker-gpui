//! macOS: finding a process by name and reading its memory through its Mach task port.
//!
//! `task_for_pid` hands out the port only to root, or to a reader signed with the
//! `com.apple.security.cs.debugger` entitlement (and then only for a target without the hardened
//! runtime, which Unity players built for Steam are). The port is used for `mach_vm_read_overwrite`
//! only: nothing here writes to the process.

use std::ffi::c_void;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use super::{FileVersion, path_matches};
use crate::error::{Error, Result};
use crate::mem::reader::Memory;

type MachPort = u32;
type KernReturn = i32;

const KERN_SUCCESS: KernReturn = 0;
const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;

unsafe extern "C" {
    static mach_task_self_: MachPort;
    fn task_for_pid(target: MachPort, pid: i32, task: *mut MachPort) -> KernReturn;
    fn pid_for_task(task: MachPort, pid: *mut i32) -> KernReturn;
    fn mach_vm_read_overwrite(task: MachPort, address: u64, size: u64, data: u64, out_size: *mut u64) -> KernReturn;
    fn mach_port_deallocate(task: MachPort, name: MachPort) -> KernReturn;
    fn proc_listallpids(buffer: *mut c_void, buffer_size: i32) -> i32;
    fn proc_pidpath(pid: i32, buffer: *mut c_void, buffer_size: u32) -> i32;
    fn kill(pid: i32, signal: i32) -> i32;
}

fn task_self() -> MachPort {
    // SAFETY: `mach_task_self_` is set by the system before `main` and never changes.
    unsafe { mach_task_self_ }
}

/// A process opened for reading its memory. The task port is released when this is dropped.
pub struct Process {
    pid: u32,
    task: MachPort,
}

// SAFETY: a task port is a plain integer name valid in this whole task; Mach calls on it are thread-safe.
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Process {
    /// Find the first running process whose executable file is `name` (ignoring case and a `.exe`)
    /// and open it for reading. On macOS the game's executable is `The Bazaar`.
    pub fn open_by_name(name: &str) -> Result<Self> {
        match find_pid(name) {
            Some(pid) => Self::open(pid),
            None => Err(Error::ProcessNotFound(name.to_owned())),
        }
    }

    /// Open the process with this id for reading only.
    pub fn open(pid: u32) -> Result<Self> {
        let mut task: MachPort = 0;
        // SAFETY: `task` outlives the call.
        let kr = unsafe { task_for_pid(task_self(), pid as i32, &mut task) };
        if kr != KERN_SUCCESS {
            return Err(Error::OpenProcess { pid, code: kr as u32 });
        }
        Ok(Self { pid, task })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn task(&self) -> MachPort {
        self.task
    }

    /// Whether the process is still running.
    pub fn is_running(&self) -> bool {
        let mut pid = 0;
        // SAFETY: `pid` outlives the call; signal 0 only checks that the process exists.
        unsafe { pid_for_task(self.task, &mut pid) == KERN_SUCCESS && pid as u32 == self.pid && kill(pid, 0) == 0 }
    }

    /// The path of the process's executable.
    pub fn image_path(&self) -> Result<PathBuf> {
        pid_path(self.pid as i32).ok_or(Error::ProcessGone)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: the port came from a successful `task_for_pid` and is released once.
        unsafe { mach_port_deallocate(task_self(), self.task) };
    }
}

impl Memory for Process {
    fn read(&self, address: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        let mut read = 0u64;
        // SAFETY: `buffer` is valid for `buffer.len()` bytes and `read` outlives the call.
        let kr = unsafe { mach_vm_read_overwrite(self.task, address, buffer.len() as u64, buffer.as_mut_ptr() as u64, &mut read) };
        if kr != KERN_SUCCESS {
            return Err(std::io::Error::other(format!("mach_vm_read_overwrite failed (kern_return {kr})")));
        }
        if read != buffer.len() as u64 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, format!("partial read: {read} of {} bytes", buffer.len())));
        }
        Ok(())
    }
}

/// The task port of `process`, for the module listing.
pub(crate) fn task_port(process: &Process) -> MachPort {
    process.task()
}

fn pid_path(pid: i32) -> Option<PathBuf> {
    let mut buffer = vec![0u8; PROC_PIDPATHINFO_MAXSIZE];
    // SAFETY: `buffer` holds the number of bytes passed.
    let len = unsafe { proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if len <= 0 {
        return None;
    }
    buffer.truncate(len as usize);
    Some(PathBuf::from(std::ffi::OsString::from_vec(buffer)))
}

fn find_pid(name: &str) -> Option<u32> {
    // SAFETY: a null buffer asks for the number of processes.
    let count = unsafe { proc_listallpids(std::ptr::null_mut(), 0) };
    if count <= 0 {
        return None;
    }
    // Room for processes started since the count was taken.
    let mut pids = vec![0i32; count as usize + 64];
    // SAFETY: `pids` holds the number of bytes passed.
    let count = unsafe { proc_listallpids(pids.as_mut_ptr().cast(), (pids.len() * size_of::<i32>()) as i32) };
    if count <= 0 {
        return None;
    }
    pids.truncate(count as usize);
    pids.into_iter().filter(|&pid| pid > 0).find(|&pid| pid_path(pid).is_some_and(|path| path_matches(&path, name))).map(|pid| pid as u32)
}

/// The Unity version of the player whose executable is at `path` (`<App>.app/Contents/MacOS/<exe>`),
/// from the `Unity Player version 6000.3.27f2` its `Info.plist` carries. `None` if there is none.
pub fn file_version(path: &Path) -> Option<FileVersion> {
    let plist = path.parent()?.parent()?.join("Info.plist");
    unity_player_version(&std::fs::read_to_string(plist).ok()?)
}

/// The version in `Unity Player version 6000.3.27f2 (…)`: 6000.3.27, build 2 (the number after the letter).
pub(super) fn unity_player_version(text: &str) -> Option<FileVersion> {
    let (_, rest) = text.split_once("Unity Player version ")?;
    let version: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '.').collect();
    let mut parts = version.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let last = parts.next()?;
    let patch_digits: String = last.chars().take_while(char::is_ascii_digit).collect();
    let build_digits: String = last.chars().skip(patch_digits.len()).skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    Some(FileVersion { major, minor, patch: patch_digits.parse().ok()?, build: build_digits.parse().unwrap_or(0) })
}
