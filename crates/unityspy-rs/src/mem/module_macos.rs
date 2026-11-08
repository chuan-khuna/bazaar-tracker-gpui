//! macOS: listing the images (executable and dylibs) loaded in a process, from dyld's own list.
//!
//! `task_info(TASK_DYLD_INFO)` gives the address of the process's `dyld_all_image_infos`, whose
//! `infoArray` holds one `dyld_image_info` (load address, path, modification date) per image.

use std::path::PathBuf;

use super::Module;
use crate::error::{Error, Result};
use crate::mem::process::{Process, task_port};
use crate::mem::reader::MemoryExt;

const TASK_DYLD_INFO: i32 = 17;
/// `TASK_DYLD_INFO_COUNT`: `task_dyld_info` in 32-bit words (it is packed to 4 bytes).
const TASK_DYLD_INFO_COUNT: u32 = 5;
/// `sizeof(struct dyld_image_info)`: load address, path pointer, modification date.
const IMAGE_INFO_SIZE: u64 = 24;
/// More images than this means the list is not what we think it is.
const MAX_IMAGES: u32 = 65_536;
/// How many times to look again while dyld is changing the list (it is null meanwhile).
const LIST_RETRIES: usize = 20;

unsafe extern "C" {
    fn task_info(task: u32, flavor: i32, info: *mut u32, count: *mut u32) -> i32;
}

/// Every image loaded in `process`. `size` is 0: dyld's list does not carry it.
pub fn modules(process: &Process) -> Result<Vec<Module>> {
    let mut info = [0u32; TASK_DYLD_INFO_COUNT as usize];
    let mut count = TASK_DYLD_INFO_COUNT;
    // SAFETY: `info` holds `count` words and both outlive the call.
    if unsafe { task_info(task_port(process), TASK_DYLD_INFO, info.as_mut_ptr(), &mut count) } != 0 {
        return Err(Error::ProcessGone);
    }
    let all_image_infos = u64::from(info[0]) | (u64::from(info[1]) << 32);
    if all_image_infos == 0 {
        return Err(Error::Unexpected("the process has no dyld image list yet".into()));
    }
    // `dyld_all_image_infos`: version (u32), infoArrayCount (u32), infoArray (pointer).
    let (mut images, mut array) = (0, 0);
    for _ in 0..LIST_RETRIES {
        images = process.read_u32(all_image_infos + 4)?;
        array = process.read_ptr(all_image_infos + 8)?;
        if array != 0 {
            break;
        }
        std::thread::yield_now();
    }
    if array == 0 {
        return Err(Error::Unexpected("dyld is changing the image list".into()));
    }
    if images > MAX_IMAGES {
        return Err(Error::Implausible { what: "dyld image count", value: i64::from(images) });
    }
    let entries = process.read_bytes(array, images as usize * IMAGE_INFO_SIZE as usize)?;
    let mut found = Vec::with_capacity(images as usize);
    for entry in entries.as_chunks::<{ IMAGE_INFO_SIZE as usize }>().0 {
        let base = u64::from_le_bytes(entry[0..8].try_into().expect("8 bytes"));
        let path_address = u64::from_le_bytes(entry[8..16].try_into().expect("8 bytes"));
        if base == 0 || path_address == 0 {
            continue;
        }
        let Ok(path) = process.read_c_string(path_address) else { continue };
        let path = PathBuf::from(path);
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        found.push(Module { name, path, base, size: 0 });
    }
    Ok(found)
}
