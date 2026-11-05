//! Where The Bazaar keeps `Player.log`.

use std::path::{Path, PathBuf};

/// The game's data folder, which holds its card data (`prod/cache/GameData.db`). On Windows it also
/// holds `Player.log`: `<profile>\AppData\LocalLow\Tempo Storm\The Bazaar`.
#[cfg(not(target_os = "macos"))]
pub fn game_folder_in(user_profile: &Path) -> PathBuf {
    user_profile.join("AppData").join("LocalLow").join("Tempo Storm").join("The Bazaar")
}

/// The game's data folder, which holds its card data (`prod/cache/GameData.db`):
/// `~/Library/Application Support/com.TempoStorm.TheBazaar`.
#[cfg(target_os = "macos")]
pub fn game_folder_in(user_profile: &Path) -> PathBuf {
    user_profile.join("Library").join("Application Support").join("com.TempoStorm.TheBazaar")
}

/// The default `Player.log` for this user profile: in the game's data folder.
#[cfg(not(target_os = "macos"))]
pub fn default_path_in(user_profile: &Path) -> PathBuf {
    game_folder_in(user_profile).join("Player.log")
}

/// The default `Player.log` for this user: Unity's log folder,
/// `~/Library/Logs/Tempo Storm/The Bazaar/Player.log`.
#[cfg(target_os = "macos")]
pub fn default_path_in(user_profile: &Path) -> PathBuf {
    user_profile.join("Library").join("Logs").join("Tempo Storm").join("The Bazaar").join("Player.log")
}

/// The user's profile folder (`USERPROFILE` on Windows, `HOME` elsewhere), if the environment says
/// where it is.
pub fn user_profile() -> Option<PathBuf> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(variable).filter(|value| !value.is_empty()).map(PathBuf::from)
}

/// The log to follow: the `log_path` setting if it holds anything but whitespace, otherwise the
/// game's default under `user_profile`. With no profile either, the bare file name (which will not
/// be found, so the follower just waits).
pub fn resolve(setting: &str, user_profile: Option<&Path>) -> PathBuf {
    if !setting.trim().is_empty() {
        return PathBuf::from(setting.trim());
    }
    user_profile.map_or_else(|| PathBuf::from("Player.log"), default_path_in)
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
