use std::path::Path;

use super::*;
use crate::error::Error;
use crate::mem::reader::MemoryExt;

#[test]
fn an_exe_name_matches_with_or_without_extension_and_ignoring_case() {
    assert!(exe_matches("TheBazaar.exe", "TheBazaar"));
    assert!(exe_matches("TheBazaar.exe", "thebazaar.EXE"));
    assert!(exe_matches("thebazaar", "TheBazaar.exe"));
    assert!(!exe_matches("TheBazaarLauncher.exe", "TheBazaar"));
}

#[cfg(windows)]
#[test]
fn a_failed_read_carries_the_windows_error() {
    use super::platform::check_read;
    let error = check_read(false, 0, 8, || 299).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(299));
}

#[cfg(windows)]
#[test]
fn a_partial_read_is_an_error() {
    use super::platform::check_read;
    assert!(check_read(true, 4, 8, || 0).is_err());
    assert!(check_read(true, 8, 8, || 0).is_ok());
}

// On macOS a process may read itself through its own task port without any entitlement.
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

#[cfg(windows)]
#[test]
fn a_system_dll_has_a_file_version() {
    let version = file_version(Path::new(r"C:\Windows\System32\kernel32.dll")).unwrap();
    assert!(version.major >= 6);
}

#[cfg(windows)]
#[test]
fn a_file_without_a_version_resource_has_none() {
    assert_eq!(file_version(Path::new(r"C:\no\such\file.exe")), None);
}

#[test]
fn an_executable_path_matches_by_its_file_name() {
    assert!(path_matches(Path::new("/Applications/The Bazaar.app/Contents/MacOS/The Bazaar"), "The Bazaar"));
    assert!(!path_matches(Path::new("/Applications/The Bazaar.app/Contents/MacOS/The Bazaar Helper"), "The Bazaar"));
}

#[cfg(target_os = "macos")]
#[test]
fn the_unity_version_is_read_from_the_player_info_string() {
    let plist = "<key>CFBundleGetInfoString</key>\n<string>Unity Player version 6000.3.27f2 (22188f149405). (c) 2005-2026</string>";
    assert_eq!(platform::unity_player_version(plist), Some(FileVersion { major: 6000, minor: 3, patch: 27, build: 2 }));
    assert_eq!(platform::unity_player_version("Unity Player version 2022.3.42f1"), Some(FileVersion { major: 2022, minor: 3, patch: 42, build: 1 }));
    assert_eq!(platform::unity_player_version("<string>Something else</string>"), None);
}

#[cfg(target_os = "macos")]
#[test]
fn a_player_without_an_info_plist_has_no_version() {
    assert_eq!(file_version(Path::new("/no/such/App.app/Contents/MacOS/App")), None);
}
