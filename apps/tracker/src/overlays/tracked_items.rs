//! The Tracked items pop-up: type to find an item in the game's catalog, Up and Down to move, Enter
//! (or a click) to track or untrack it, Escape to close. The rows themselves are worked out by the
//! app (`tracking::item_odds::candidates`) from what is typed here.

#[derive(Default)]
pub struct TrackedItemsOverlay {
    pub query: String,
    /// Which row is highlighted.
    pub selected: usize,
}

/// What a key press in the pop-up asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Close,
    /// Track the row at this index, or untrack it if it is tracked.
    Toggle(usize),
    /// Nothing to change, apart from the pop-up itself.
    None,
}

impl TrackedItemsOverlay {
    /// `key` is gpui's name for the key ("up", "enter", "a"); `typed` is the text it types, if any;
    /// `rows` is how many rows are listed now.
    pub fn press(&mut self, key: &str, typed: Option<&str>, rows: usize) -> Outcome {
        match key {
            "escape" => Outcome::Close,
            "enter" if self.selected < rows => Outcome::Toggle(self.selected),
            "up" | "down" if rows > 0 => {
                self.selected = if key == "down" { (self.selected + 1) % rows } else { (self.selected + rows - 1) % rows };
                Outcome::None
            }
            "backspace" => {
                self.query.pop();
                self.selected = 0;
                Outcome::None
            }
            _ => {
                if let Some(text) = typed.filter(|text| !text.chars().any(char::is_control)) {
                    self.query.push_str(text);
                    self.selected = 0;
                }
                Outcome::None
            }
        }
    }
}

#[cfg(test)]
#[path = "tracked_items_tests.rs"]
mod tests;
