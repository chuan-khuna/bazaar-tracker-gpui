//! `settings.toml`: the settings the player can change, loaded with defaults for anything missing
//! and saved as soon as one changes.
//!
//! A file that cannot be read or parsed is not an error for the app: the defaults are used in
//! memory and the file is left alone until a setting changes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "settings.toml";

/// The least and most the sidebar can be dragged to, and its width when the app first opens (all
/// in pixels, including the gap beside it).
pub const SIDEBAR_MIN_WIDTH: u32 = 220;
pub const SIDEBAR_MAX_WIDTH: u32 = 520;
pub const SIDEBAR_DEFAULT_WIDTH: u32 = 248;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The name of the theme. An unknown name gives Rosé Pine.
    pub theme: String,
    /// Where `Player.log` is, if the game keeps it somewhere unusual. Empty means the usual place.
    pub log_path: String,
    pub run_sync: bool,
    pub show_run_duration: bool,
    pub window: WindowSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: "Rosé Pine".into(), log_path: String::new(), run_sync: true, show_run_duration: false, window: WindowSettings::default() }
    }
}

/// How the window's panels are laid out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    pub sidebar_open: bool,
    /// Kept between `SIDEBAR_MIN_WIDTH` and `SIDEBAR_MAX_WIDTH`: use `set_sidebar_width`.
    pub sidebar_width: u32,
    pub detail_open: bool,
    pub stash_open: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self { sidebar_open: true, sidebar_width: SIDEBAR_DEFAULT_WIDTH, detail_open: true, stash_open: true }
    }
}

impl WindowSettings {
    pub fn set_sidebar_width(&mut self, width: u32) {
        self.sidebar_width = clamp_sidebar_width(width);
    }
}

/// `width`, brought into the range the sidebar can have.
pub fn clamp_sidebar_width(width: u32) -> u32 {
    width.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

impl Settings {
    /// Put hand-edited values back in their range.
    fn normalise(&mut self) {
        let width = self.window.sidebar_width;
        self.window.set_sidebar_width(width);
    }

    fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("settings are always representable as TOML")
    }
}

/// How loading went, for the caller to report.
#[derive(Debug, PartialEq)]
pub enum Load {
    /// The file was read (keys it lacks have their defaults).
    Read,
    /// There was no file: the defaults were used, and written out.
    Created,
    /// The file could not be read or parsed: the defaults are used in memory, and the file is
    /// untouched until a setting changes. Holds what went wrong.
    Unusable(String),
}

/// The settings and the file they live in.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
}

impl SettingsStore {
    /// Load `settings.toml` from `data_dir`.
    pub fn load(data_dir: &Path) -> (Self, Load) {
        let path = data_dir.join(FILE_NAME);
        let (settings, outcome) = match fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(mut settings) => {
                    settings.normalise();
                    (settings, Load::Read)
                }
                Err(error) => (Settings::default(), Load::Unusable(format!("{FILE_NAME} could not be parsed: {}", error.message()))),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Settings::default(), Load::Created),
            Err(error) => (Settings::default(), Load::Unusable(format!("{FILE_NAME} could not be read: {error}"))),
        };
        let store = Self { path, settings };
        if outcome == Load::Created {
            // Best effort: the app works without the file, and the next change tries again.
            let _ = store.save();
        }
        (store, outcome)
    }

    pub fn get(&self) -> &Settings {
        &self.settings
    }

    /// Change the settings, and save them if anything changed.
    pub fn update(&mut self, change: impl FnOnce(&mut Settings)) -> io::Result<()> {
        let before = self.settings.clone();
        change(&mut self.settings);
        self.settings.normalise();
        if self.settings == before { Ok(()) } else { self.save() }
    }

    fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        // Written beside the file and moved over it, so a crash never leaves half a file.
        let temporary = self.path.with_extension("toml.tmp");
        fs::write(&temporary, self.settings.to_toml())?;
        fs::rename(&temporary, &self.path)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
