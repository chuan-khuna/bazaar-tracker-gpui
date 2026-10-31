//! The settings overlay. It has no settings in it yet; it closes with Escape, its ✕ or a click outside.

#[derive(Default)]
pub struct SettingsOverlay;

/// What a key press in the settings overlay asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Close,
    /// Nothing to change.
    None,
}

impl SettingsOverlay {
    /// `key` is gpui's name for the key ("escape", "a").
    pub fn press(&mut self, key: &str) -> Outcome {
        match key {
            "escape" => Outcome::Close,
            _ => Outcome::None,
        }
    }
}

#[cfg(test)]
#[path = "settings_overlay_tests.rs"]
mod tests;
