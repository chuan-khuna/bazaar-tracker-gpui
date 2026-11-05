//! The settings overlay. It closes with Escape, its ✕ or a click outside.
//!
//! Its Data section's Clear data asks first: the player types [`CLEAR_WORD`] and presses Enter or
//! clicks Clear. While it asks, typing goes to it and Escape stops asking.

/// The word typed to confirm Clear data (in any case).
pub const CLEAR_WORD: &str = "delete";

/// The longest the typed confirmation can get; more is not taken.
const MAX_TYPED: usize = 32;

#[derive(Default)]
pub struct SettingsOverlay {
    /// While Clear data asks, what has been typed to confirm it.
    pub clear_confirm: Option<String>,
}

/// What a key press in the settings overlay asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Close,
    /// The confirmation was typed and Enter pressed: clear the data.
    ClearData,
    /// Nothing to change, apart from the overlay itself.
    None,
}

impl SettingsOverlay {
    /// Open with Clear data already asking, as the Clear data command does.
    pub fn asking_to_clear() -> Self {
        Self { clear_confirm: Some(String::new()) }
    }

    pub fn ask_to_clear(&mut self) {
        self.clear_confirm = Some(String::new());
    }

    pub fn stop_asking(&mut self) {
        self.clear_confirm = None;
    }

    /// Whether [`CLEAR_WORD`] has been typed.
    pub fn clear_confirmed(&self) -> bool {
        self.clear_confirm.as_deref().is_some_and(|typed| typed.trim().eq_ignore_ascii_case(CLEAR_WORD))
    }

    /// `key` is gpui's name for the key ("escape", "a"); `typed` is the text it types, if any.
    pub fn press(&mut self, key: &str, typed: Option<&str>) -> Outcome {
        let Some(text) = self.clear_confirm.as_mut() else {
            return if key == "escape" { Outcome::Close } else { Outcome::None };
        };
        match key {
            "escape" => self.stop_asking(),
            "enter" => return if self.clear_confirmed() { Outcome::ClearData } else { Outcome::None },
            "backspace" => {
                text.pop();
            }
            _ => {
                if let Some(typed) = typed.filter(|typed| !typed.chars().any(char::is_control))
                    && text.chars().count() + typed.chars().count() <= MAX_TYPED
                {
                    text.push_str(typed);
                }
            }
        }
        Outcome::None
    }
}

#[cfg(test)]
#[path = "settings_overlay_tests.rs"]
mod tests;
