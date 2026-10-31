use super::*;
use crate::mem::reader::MemoryExt;

#[test]
fn an_exe_name_matches_with_or_without_extension_and_ignoring_case() {
    assert!(exe_matches("TheBazaar.exe", "TheBazaar"));
    assert!(exe_matches("TheBazaar.exe", "thebazaar.EXE"));
    assert!(exe_matches("thebazaar", "TheBazaar.exe"));
    assert!(!exe_matches("TheBazaarLauncher.exe", "TheBazaar"));
}

#[test]
fn a_failed_read_carries_the_windows_error() {
    let error = check_read(false, 0, 8, || 299).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(299));
}

#[test]
fn a_partial_read_is_an_error() {
    assert!(check_read(true, 4, 8, || 0).is_err());
    assert!(check_read(true, 8, 8, || 0).is_ok());
}

#[test]
fn this_process_can_be_opened_and_read_by_pid() {
    let process = Process::open(std::process::id()).unwrap();
    let value: u64 = 0x1122_3344_5566_7788;
    assert_eq!(process.read_u64(&value as *const u64 as u64).unwrap(), value);
    assert!(process.is_running());
}

#[test]
fn this_process_can_be_found_by_name() {
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_string_lossy().into_owned();
    let process = Process::open_by_name(&name).unwrap();
    assert!(process.pid() != 0);
    assert_eq!(process.image_path().unwrap().file_name(), exe.file_name());
}

#[test]
fn reading_unmapped_memory_in_a_live_process_fails() {
    let process = Process::open(std::process::id()).unwrap();
    assert!(process.read_u64(8).is_err());
}

#[test]
fn a_missing_process_is_named_in_the_error() {
    let error = Process::open_by_name("no-such-game-5f3a").err().unwrap();
    assert!(matches!(&error, Error::ProcessNotFound(name) if name == "no-such-game-5f3a"));
    assert!(error.to_string().contains("no-such-game-5f3a"));
}

#[test]
fn a_system_dll_has_a_file_version() {
    let version = file_version(Path::new(r"C:\Windows\System32\kernel32.dll")).unwrap();
    assert!(version.major >= 6);
}

#[test]
fn a_file_without_a_version_resource_has_none() {
    assert_eq!(file_version(Path::new(r"C:\no\such\file.exe")), None);
}
