//! Where the app keeps its own data: `settings.toml`, later `history.db`, and `logs/`.
//!
//! In order: `BAZAAR_TRACKER_DATA_DIR` if set; otherwise, in a debug build, the repo's
//! `.bazaar-tracker/` (fixed at compile time from the manifest directory); otherwise
//! `%APPDATA%\BazaarTracker\`.

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
        Path::new(env!("CARGO_MANIFEST_DIR")),
        std::env::var_os("APPDATA").map(PathBuf::from).as_deref(),
    );
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
#[path = "data_dir_tests.rs"]
mod tests;
