//! The pop-up of one item's saved odds. It closes with Escape, its ✕ or a click outside.

use crate::tracking::item_odds::Wanted;

/// Which item's odds are shown, and the day whose column is marked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemOddsOverlay {
    pub item: Wanted,
    pub day: Option<u32>,
}

/// What a key press in the pop-up asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Close,
    /// Nothing to change.
    None,
}

impl ItemOddsOverlay {
    /// `key` is gpui's name for the key ("escape", "a").
    pub fn press(&mut self, key: &str) -> Outcome {
        match key {
            "escape" => Outcome::Close,
            _ => Outcome::None,
        }
    }
}

#[cfg(test)]
#[path = "item_odds_tests.rs"]
mod tests;
