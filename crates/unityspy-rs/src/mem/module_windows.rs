//! Windows: listing the modules (exe and DLLs) loaded in a process.

use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use super::Module;
use crate::error::{Error, Result};
use crate::mem::process::{Process, RawHandle};

const LIST_MODULES_ALL: u32 = 0x03;

#[repr(C)]
struct ModuleInfo {
    base_of_dll: *mut c_void,
    size_of_image: u32,
    entry_point: *mut c_void,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn K32EnumProcessModulesEx(process: RawHandle, modules: *mut RawHandle, bytes: u32, needed: *mut u32, filter: u32) -> i32;
    fn K32GetModuleFileNameExW(process: RawHandle, module: RawHandle, name: *mut u16, size: u32) -> u32;
    fn K32GetModuleInformation(process: RawHandle, module: RawHandle, info: *mut ModuleInfo, size: u32) -> i32;
}

/// Every module loaded in `process`.
pub fn modules(process: &Process) -> Result<Vec<Module>> {
    let handle = process.raw_handle();
    let mut handles: Vec<RawHandle> = vec![std::ptr::null_mut(); 1024];
    loop {
        let mut needed = 0u32;
        let bytes = (handles.len() * std::mem::size_of::<RawHandle>()) as u32;
        // SAFETY: `handles` is valid for `bytes` bytes and `needed` outlives the call.
        let ok = unsafe { K32EnumProcessModulesEx(handle, handles.as_mut_ptr(), bytes, &mut needed, LIST_MODULES_ALL) };
        if ok == 0 {
            return Err(Error::ProcessGone);
        }
        if needed > bytes {
            handles.resize(needed as usize / std::mem::size_of::<RawHandle>() + 16, std::ptr::null_mut());
            continue;
        }
        handles.truncate(needed as usize / std::mem::size_of::<RawHandle>());
        break;
    }
    let mut found = Vec::with_capacity(handles.len());
    for module in handles {
        let mut name = vec![0u16; 1024];
        // SAFETY: `name` holds the number of units passed.
        let len = unsafe { K32GetModuleFileNameExW(handle, module, name.as_mut_ptr(), name.len() as u32) } as usize;
        if len == 0 {
            continue;
        }
        let path = PathBuf::from(std::ffi::OsString::from_wide(&name[..len]));
        let mut info = ModuleInfo { base_of_dll: std::ptr::null_mut(), size_of_image: 0, entry_point: std::ptr::null_mut() };
        // SAFETY: `info` is a valid `MODULEINFO` of the size passed.
        if unsafe { K32GetModuleInformation(handle, module, &mut info, std::mem::size_of::<ModuleInfo>() as u32) } == 0 {
            continue;
        }
        let file_name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        found.push(Module { name: file_name, path, base: info.base_of_dll as u64, size: u64::from(info.size_of_image) });
    }
    Ok(found)
}
