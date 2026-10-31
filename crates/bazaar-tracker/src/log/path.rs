//! Where The Bazaar keeps `Player.log`.

use std::path::{Path, PathBuf};

/// The game's data folder: `<profile>\AppData\LocalLow\Tempo Storm\The Bazaar`.
pub fn game_folder_in(user_profile: &Path) -> PathBuf {
    user_profile.join("AppData").join("LocalLow").join("Tempo Storm").join("The Bazaar")
}

/// The default `Player.log` for this user profile.
pub fn default_path_in(user_profile: &Path) -> PathBuf {
    game_folder_in(user_profile).join("Player.log")
}

/// The user's profile folder, if the environment says where it is.
pub fn user_profile() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()).map(PathBuf::from)
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
