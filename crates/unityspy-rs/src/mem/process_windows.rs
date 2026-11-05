//! Windows: finding a process by name and opening it with `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION` only.
//!
//! Nothing here can write to the process or change it. If opening is refused, the error says so and
//! the caller may decide to ask for admin rights; this crate never asks.

use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use super::{FileVersion, exe_matches};
use crate::error::{Error, Result};
use crate::mem::reader::Memory;

pub(crate) type RawHandle = *mut c_void;

const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const INVALID_HANDLE_VALUE: RawHandle = -1isize as RawHandle;
const STILL_ACTIVE: u32 = 259;

#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    usage: u32,
    process_id: u32,
    default_heap_id: usize,
    module_id: u32,
    threads: u32,
    parent_process_id: u32,
    priority_class_base: i32,
    flags: u32,
    exe_file: [u16; 260],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> RawHandle;
    fn Process32FirstW(snapshot: RawHandle, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: RawHandle, entry: *mut ProcessEntry32W) -> i32;
    fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> RawHandle;
    fn ReadProcessMemory(process: RawHandle, base: *const c_void, buffer: *mut c_void, size: usize, read: *mut usize) -> i32;
    fn GetExitCodeProcess(process: RawHandle, code: *mut u32) -> i32;
    fn QueryFullProcessImageNameW(process: RawHandle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    fn CloseHandle(handle: RawHandle) -> i32;
    fn GetLastError() -> u32;
}

#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(path: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(path: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
    fn VerQueryValueW(block: *const c_void, sub_block: *const u16, buffer: *mut *mut c_void, len: *mut u32) -> i32;
}

/// A process opened for reading its memory. The handle is closed when this is dropped.
pub struct Process {
    pid: u32,
    handle: RawHandle,
}

// SAFETY: a process handle may be used from any thread; `ReadProcessMemory` is thread-safe.
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Process {
    /// Find the first running process whose executable is `name` (with or without `.exe`, ignoring
    /// case) and open it for reading.
    pub fn open_by_name(name: &str) -> Result<Self> {
        match find_pid(name) {
            Some(pid) => Self::open(pid),
            None => Err(Error::ProcessNotFound(name.to_owned())),
        }
    }

    /// Open the process with this id for reading only.
    pub fn open(pid: u32) -> Result<Self> {
        // SAFETY: plain call with no pointers.
        let handle = unsafe { OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            // SAFETY: read straight after the call it reports on.
            return Err(Error::OpenProcess { pid, code: unsafe { GetLastError() } });
        }
        Ok(Self { pid, handle })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn raw_handle(&self) -> RawHandle {
        self.handle
    }

    /// Whether the process is still running.
    pub fn is_running(&self) -> bool {
        let mut code = 0;
        // SAFETY: `code` outlives the call.
        unsafe { GetExitCodeProcess(self.handle, &mut code) != 0 && code == STILL_ACTIVE }
    }

    /// The path of the process's executable.
    pub fn image_path(&self) -> Result<PathBuf> {
        let mut buffer = vec![0u16; 1024];
        let mut len = buffer.len() as u32;
        // SAFETY: `buffer` holds `len` units and both outlive the call.
        let ok = unsafe { QueryFullProcessImageNameW(self.handle, 0, buffer.as_mut_ptr(), &mut len) };
        if ok == 0 {
            return Err(Error::ProcessGone);
        }
        Ok(PathBuf::from(std::ffi::OsString::from_wide(&buffer[..len as usize])))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful `OpenProcess` and is closed once.
        unsafe { CloseHandle(self.handle) };
    }
}

impl Memory for Process {
    fn read(&self, address: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        let mut read = 0usize;
        // SAFETY: `buffer` is valid for `buffer.len()` bytes and `read` outlives the call.
        let ok = unsafe { ReadProcessMemory(self.handle, address as usize as *const c_void, buffer.as_mut_ptr().cast(), buffer.len(), &mut read) };
        check_read(ok != 0, read, buffer.len(), || {
            // SAFETY: read straight after the call it reports on.
            unsafe { GetLastError() }
        })
    }
}

/// Turn the outcome of one `ReadProcessMemory` into a result. A partial read is an error.
pub(super) fn check_read(ok: bool, read: usize, wanted: usize, last_error: impl FnOnce() -> u32) -> std::io::Result<()> {
    if !ok {
        return Err(std::io::Error::from_raw_os_error(last_error() as i32));
    }
    if read != wanted {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, format!("partial read: {read} of {wanted} bytes")));
    }
    Ok(())
}

fn find_pid(name: &str) -> Option<u32> {
    // SAFETY: plain call with no pointers.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
        return None;
    }
    let mut entry = ProcessEntry32W {
        size: std::mem::size_of::<ProcessEntry32W>() as u32,
        usage: 0,
        process_id: 0,
        default_heap_id: 0,
        module_id: 0,
        threads: 0,
        parent_process_id: 0,
        priority_class_base: 0,
        flags: 0,
        exe_file: [0; 260],
    };
    let mut found = None;
    // SAFETY: `entry` is a valid, correctly sized `PROCESSENTRY32W` for every call.
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while more {
        let len = entry.exe_file.iter().position(|&unit| unit == 0).unwrap_or(entry.exe_file.len());
        let exe = String::from_utf16_lossy(&entry.exe_file[..len]);
        if exe_matches(&exe, name) {
            found = Some(entry.process_id);
            break;
        }
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    // SAFETY: the snapshot handle is closed once.
    unsafe { CloseHandle(snapshot) };
    found
}

/// The `FileVersion` of the executable or DLL at `path`, or `None` if it has none. Unity stamps a
/// player's executable with the editor's version, so `2022.3.42f1` reads as 2022.3.42.
pub fn file_version(path: &Path) -> Option<FileVersion> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut ignored = 0;
    // SAFETY: `wide` is NUL-terminated and outlives the calls; `data` is as long as the size asked for.
    unsafe {
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut ignored);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, data.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let root: Vec<u16> = "\\".encode_utf16().chain(Some(0)).collect();
        let mut block: *mut c_void = std::ptr::null_mut();
        let mut len = 0;
        if VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), &mut block, &mut len) == 0 || block.is_null() || len < 16 {
            return None;
        }
        // VS_FIXEDFILEINFO: signature, struct version, then the file version's two halves.
        let fields = std::slice::from_raw_parts(block as *const u32, 4);
        if fields[0] != 0xFEEF_04BD {
            return None;
        }
        let (ms, ls) = (fields[2], fields[3]);
        Some(FileVersion { major: (ms >> 16) as u16, minor: ms as u16, patch: (ls >> 16) as u16, build: ls as u16 })
    }
}

