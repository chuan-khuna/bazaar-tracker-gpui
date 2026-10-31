//! The theme picker, like Zed: type to filter the themes by name (or "dark"/"light"), Up and Down
//! to move (each move previews that theme), Enter to choose, Escape to go back to the theme from
//! before the picker opened.

use crate::theme::{Theme, search};

pub struct ThemePicker {
    pub query: String,
    /// Which of the matches is highlighted.
    pub selected: usize,
    /// The theme before the picker opened, put back on Escape.
    pub original: &'static Theme,
}

/// What a key press in the picker asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Show this theme while the picker stays open.
    Preview(&'static Theme),
    /// Keep this theme, save it and close the picker.
    Choose(&'static Theme),
    /// Show this theme (the one from before the picker opened) and close the picker.
    Restore(&'static Theme),
    /// Nothing to change, apart from the picker itself.
    None,
}

impl ThemePicker {
    pub fn new(current: &'static Theme) -> Self {
        let selected = search("").iter().position(|theme| std::ptr::eq(*theme, current)).unwrap_or(0);
        Self { query: String::new(), selected, original: current }
    }

    pub fn matches(&self) -> Vec<&'static Theme> {
        search(&self.query)
    }

    fn preview_selected(&self) -> Outcome {
        self.matches().get(self.selected).map_or(Outcome::None, |theme| Outcome::Preview(theme))
    }

    /// `key` is gpui's name for the key ("up", "enter", "a"); `typed` is the text it types, if any.
    pub fn press(&mut self, key: &str, typed: Option<&str>) -> Outcome {
        let count = self.matches().len();
        match key {
            "escape" => Outcome::Restore(self.original),
            "enter" => self.matches().get(self.selected).map_or(Outcome::None, |theme| Outcome::Choose(theme)),
            "up" | "down" if count > 0 => {
                self.selected = if key == "down" { (self.selected + 1) % count } else { (self.selected + count - 1) % count };
                self.preview_selected()
            }
            "backspace" => {
                self.query.pop();
                self.selected = 0;
                self.preview_selected()
            }
            _ => match typed.filter(|text| !text.chars().any(char::is_control)) {
                Some(text) => {
                    self.query.push_str(text);
                    self.selected = 0;
                    self.preview_selected()
                }
                None => Outcome::None,
            },
        }
    }
}

#[cfg(test)]
#[path = "theme_picker_tests.rs"]
mod tests;
