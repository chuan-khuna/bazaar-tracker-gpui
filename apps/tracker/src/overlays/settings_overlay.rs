//! The settings overlay. It closes with Escape, its ✕ or a click outside.
//!
//! Its Data section's Clear data asks first: the player picks what to clear (the database, the
//! card art, or both), types [`CLEAR_WORD`] and presses Enter or clicks Clear. While it asks,
//! typing goes to it and Escape stops asking.

/// The word typed to confirm Clear data (in any case).
pub const CLEAR_WORD: &str = "delete";

/// The longest the typed confirmation can get; more is not taken.
const MAX_TYPED: usize = 32;

/// What Clear data is asking to clear, and what has been typed to confirm it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClearAsk {
    pub typed: String,
    /// The database: the runs and the shop and item odds.
    pub database: bool,
    /// The card art folder.
    pub art: bool,
}

impl Default for ClearAsk {
    fn default() -> Self {
        Self { typed: String::new(), database: true, art: true }
    }
}

impl ClearAsk {
    /// Whether something is picked and [`CLEAR_WORD`] has been typed.
    pub fn confirmed(&self) -> bool {
        (self.database || self.art) && self.typed.trim().eq_ignore_ascii_case(CLEAR_WORD)
    }
}

#[derive(Default)]
pub struct SettingsOverlay {
    /// While Clear data asks, what it asks about.
    pub clear: Option<ClearAsk>,
}

/// What a key press in the settings overlay asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Close,
    /// The confirmation was typed and Enter pressed: clear what was picked.
    ClearData { database: bool, art: bool },
    /// Nothing to change, apart from the overlay itself.
    None,
}

impl SettingsOverlay {
    /// Open with Clear data already asking, as the Clear data command does.
    pub fn asking_to_clear() -> Self {
        Self { clear: Some(ClearAsk::default()) }
    }

    pub fn ask_to_clear(&mut self) {
        self.clear = Some(ClearAsk::default());
    }

    pub fn stop_asking(&mut self) {
        self.clear = None;
    }

    /// `key` is gpui's name for the key ("escape", "a"); `typed` is the text it types, if any.
    pub fn press(&mut self, key: &str, typed: Option<&str>) -> Outcome {
        let Some(ask) = self.clear.as_mut() else {
            return if key == "escape" { Outcome::Close } else { Outcome::None };
        };
        match key {
            "escape" => self.stop_asking(),
            "enter" => return if ask.confirmed() { Outcome::ClearData { database: ask.database, art: ask.art } } else { Outcome::None },
            "backspace" => {
                ask.typed.pop();
            }
            _ => {
                if let Some(typed) = typed.filter(|typed| !typed.chars().any(char::is_control))
                    && ask.typed.chars().count() + typed.chars().count() <= MAX_TYPED
                {
                    ask.typed.push_str(typed);
                }
            }
        }
        Outcome::None
    }
}

#[cfg(test)]
#[path = "settings_overlay_tests.rs"]
mod tests;
