//! Where the app keeps its own data: `settings.toml`, later `history.db`, and `logs/`.
//!
//! In order: `BAZAAR_TRACKER_DATA_DIR` if set; otherwise, in a debug build, the repo's
//! `.bazaar-tracker/` (fixed at compile time: two folders up from `apps/tracker`); otherwise
//! `%APPDATA%\BazaarTracker\` on Windows and `~/Library/Application Support/BazaarTracker/` on macOS.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

pub const ENV_VAR: &str = "BAZAAR_TRACKER_DATA_DIR";

/// The folder inside the repo that a debug build uses.
const DEBUG_FOLDER: &str = ".bazaar-tracker";
/// The folder inside `%APPDATA%` that a release build uses.
const RELEASE_FOLDER: &str = "BazaarTracker";

/// The data folder, as a function of its inputs: the value of `BAZAAR_TRACKER_DATA_DIR` (an empty
/// value counts as unset), whether this is a debug build, the repo's directory, and `%APPDATA%`.
///
/// With no `%APPDATA%` (which Windows always sets), a release build uses `BazaarTracker` in the
/// current directory.
pub fn resolve(env_override: Option<&OsStr>, debug_build: bool, repo_dir: &Path, app_data: Option<&Path>) -> PathBuf {
    match env_override.filter(|value| !value.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None if debug_build => repo_dir.join(DEBUG_FOLDER),
        None => app_data.map_or_else(|| PathBuf::from(RELEASE_FOLDER), |dir| dir.join(RELEASE_FOLDER)),
    }
}

/// The data folder for this process, created if it is missing.
pub fn locate() -> io::Result<PathBuf> {
    let dir = resolve(
        std::env::var_os(ENV_VAR).as_deref(),
        cfg!(debug_assertions),
        repo_dir(),
        app_data().as_deref(),
    );
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// The repo's root folder, fixed at compile time: the app's manifest is in `<repo>/apps/tracker`.
fn repo_dir() -> &'static Path {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.ancestors().nth(2).unwrap_or(manifest)
}

/// Where applications keep their data: `%APPDATA%` on Windows.
#[cfg(not(target_os = "macos"))]
fn app_data() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// Where applications keep their data: `~/Library/Application Support` on macOS.
#[cfg(target_os = "macos")]
fn app_data() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|home| !home.is_empty()).map(|home| PathBuf::from(home).join("Library").join("Application Support"))
}

#[cfg(test)]
#[path = "data_dir_tests.rs"]
mod tests;
