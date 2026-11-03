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

/// The least and most the log panel can be dragged to, and its width when first opened (pixels).
pub const LOG_MIN_WIDTH: u32 = 260;
pub const LOG_MAX_WIDTH: u32 = 900;
pub const LOG_DEFAULT_WIDTH: u32 = 420;

/// The least and most the detail island can be dragged to, and its width when first opened (pixels).
pub const DETAIL_MIN_WIDTH: u32 = 240;
pub const DETAIL_MAX_WIDTH: u32 = 1200;
pub const DETAIL_DEFAULT_WIDTH: u32 = 560;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The name of the theme. An unknown name gives Rosé Pine.
    pub theme: String,
    /// Where `Player.log` is, if the game keeps it somewhere unusual. Empty means the usual place.
    pub log_path: String,
    /// The Bazaar's install folder, for extracting card art. Empty means find it in the Steam libraries.
    pub game_dir: String,
    /// Whether runs are uploaded to Bazaar DB. Off until the player turns it on; nothing is sent
    /// without a secret key either way.
    pub run_sync: bool,
    pub show_run_duration: bool,
    /// Whether item odds at an encounter choice end with the chance at any shop. Off by default:
    /// the merchants on offer are what the choice is between.
    pub show_any_shop_odds: bool,
    /// How big item cards and skill icons are drawn.
    pub card_size: CardSize,
    pub window: WindowSettings,
    pub pop_out: PopOutSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: "Rosé Pine".into(), log_path: String::new(), game_dir: String::new(), run_sync: false, show_run_duration: false, show_any_shop_odds: false, card_size: CardSize::default(), window: WindowSettings::default(), pop_out: PopOutSettings::default() }
    }
}

/// How big item cards and skill icons are drawn: Medium is 3/4 of Large, Small half of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl CardSize {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }
}

/// Which of the Live island's two odds lists shows: the chance of each watched item at the merchants
/// of an encounter choice, or the merchants likeliest to have each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OddsTab {
    #[default]
    ItemOdds,
    BestShops,
}

/// Which sections the pop-out over the game shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PopOutSettings {
    pub economy: bool,
    pub shop_odds: bool,
    pub item_odds: bool,
    pub best_shops: bool,
}

impl Default for PopOutSettings {
    fn default() -> Self {
        Self { economy: true, shop_odds: true, item_odds: true, best_shops: true }
    }
}

/// How the window's panels are laid out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    pub sidebar_open: bool,
    /// Kept between `SIDEBAR_MIN_WIDTH` and `SIDEBAR_MAX_WIDTH`: use `set_sidebar_width`.
    pub sidebar_width: u32,
    /// Kept between `DETAIL_MIN_WIDTH` and `DETAIL_MAX_WIDTH`: use `set_detail_width`.
    pub detail_width: u32,
    /// Whether the Snapshot section is unfolded.
    pub snapshot_open: bool,
    pub stash_open: bool,
    /// Whether the item odds and best shops in the Live island are unfolded, and which shows.
    pub odds_open: bool,
    pub odds_tab: OddsTab,
    /// Whether the log panel on the right is open.
    pub log_open: bool,
    /// Whether the facts pinned at the top of the log panel are unfolded.
    pub log_details_open: bool,
    /// Kept between `LOG_MIN_WIDTH` and `LOG_MAX_WIDTH`: use `set_log_width`.
    pub log_width: u32,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self { sidebar_open: true, sidebar_width: SIDEBAR_DEFAULT_WIDTH, detail_width: DETAIL_DEFAULT_WIDTH, snapshot_open: true, stash_open: true, odds_open: true, odds_tab: OddsTab::default(), log_open: false, log_details_open: true, log_width: LOG_DEFAULT_WIDTH }
    }
}

impl WindowSettings {
    pub fn set_sidebar_width(&mut self, width: u32) {
        self.sidebar_width = clamp_sidebar_width(width);
    }

    pub fn set_log_width(&mut self, width: u32) {
        self.log_width = clamp_log_width(width);
    }

    pub fn set_detail_width(&mut self, width: u32) {
        self.detail_width = clamp_detail_width(width);
    }
}

/// `width`, brought into the range the log panel can have.
pub fn clamp_log_width(width: u32) -> u32 {
    width.clamp(LOG_MIN_WIDTH, LOG_MAX_WIDTH)
}

/// `width`, brought into the range the detail island can have.
pub fn clamp_detail_width(width: u32) -> u32 {
    width.clamp(DETAIL_MIN_WIDTH, DETAIL_MAX_WIDTH)
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
        let width = self.window.log_width;
        self.window.set_log_width(width);
        let width = self.window.detail_width;
        self.window.set_detail_width(width);
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

    /// The folder `settings.toml` is in: the data folder.
    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
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
