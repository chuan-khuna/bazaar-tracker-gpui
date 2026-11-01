//! The Shop odds widget's pop-up. It closes with Escape, its ✕ or a click outside.

#[derive(Default)]
pub struct ShopOddsOverlay;

/// What a key press in the pop-up asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Close,
    /// Nothing to change.
    None,
}

impl ShopOddsOverlay {
    /// `key` is gpui's name for the key ("escape", "a").
    pub fn press(&mut self, key: &str) -> Outcome {
        match key {
            "escape" => Outcome::Close,
            _ => Outcome::None,
        }
    }
}

#[cfg(test)]
#[path = "shop_odds_tests.rs"]
mod tests;
