//! The Advisor's model picker: type to filter the models `GET /api/models` listed, Up and Down to
//! move, Enter (or a click) to use the highlighted one for the next messages, Shift+Enter to also
//! make it the default (saved in Settings), Escape to close. A model the list lacks can be typed in
//! whole: it is offered as the last row. Opened from Settings, choosing always sets the default.
//!
//! The rule: the picker starts on the saved default; choosing a model changes only what the next
//! messages are sent with, until the app restarts, unless it is set as the default.

use std::ops::Range;

use crate::tracking::advisor::client::ModelOption;

/// How many rows the picker shows at once; the window follows the highlighted row.
pub const VISIBLE_ROWS: usize = 12;

/// The longest the query can get, in characters.
const MAX_QUERY: usize = 200;

/// One row of the picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The backend's own `ADVISOR_MODEL` (no model sent); holds its id once known.
    Backend(Option<String>),
    /// A model from the list.
    Model(ModelOption),
    /// What was typed, used as a model id as it is.
    Typed(String),
}

impl Row {
    /// The model it sends; `None` for the backend's own.
    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Backend(_) => None,
            Self::Model(option) => Some(&option.id),
            Self::Typed(id) => Some(id),
        }
    }
}

/// The rows for `query`: the backend's default (while the query is empty), the models whose id or
/// name holds every word of the query (in any case), then what was typed if no model has that id.
pub fn rows(query: &str, models: &[ModelOption], backend: Option<&str>) -> Vec<Row> {
    let query = query.trim();
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_owned).collect();
    let mut rows = Vec::new();
    if words.is_empty() {
        rows.push(Row::Backend(backend.map(str::to_owned)));
    }
    rows.extend(
        models
            .iter()
            .filter(|option| {
                let id = option.id.to_lowercase();
                let name = option.name.to_lowercase();
                words.iter().all(|word| id.contains(word) || name.contains(word))
            })
            .cloned()
            .map(Row::Model),
    );
    if !query.is_empty() && !query.contains(char::is_whitespace) && !models.iter().any(|option| option.id == query) {
        rows.push(Row::Typed(query.to_owned()));
    }
    rows
}

/// Which of `count` rows to draw so that `selected` is among them: at most [`VISIBLE_ROWS`].
pub fn visible(selected: usize, count: usize) -> Range<usize> {
    let size = VISIBLE_ROWS.min(count);
    let start = selected.saturating_sub(size.saturating_sub(1)).min(count - size);
    start..start + size
}

pub struct ModelPicker {
    pub query: String,
    /// Which row is highlighted.
    pub selected: usize,
    /// Opened from Settings: choosing sets the default.
    pub for_default: bool,
}

/// What a key press in the picker asks the app to do.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Close,
    /// Use the row at this index; `default`: also save it as the default model.
    Choose { index: usize, default: bool },
    /// Nothing to change, apart from the picker itself.
    None,
}

impl ModelPicker {
    pub fn new(for_default: bool) -> Self {
        Self { query: String::new(), selected: 0, for_default }
    }

    /// `key` is gpui's name for the key ("up", "enter", "a"); `typed` is the text it types, if any;
    /// `shift` whether Shift is held; `count` how many rows are listed now.
    pub fn press(&mut self, key: &str, typed: Option<&str>, shift: bool, count: usize) -> Outcome {
        match key {
            "escape" => Outcome::Close,
            "enter" if self.selected < count => Outcome::Choose { index: self.selected, default: shift || self.for_default },
            "up" | "down" if count > 0 => {
                self.selected = if key == "down" { (self.selected + 1) % count } else { (self.selected + count - 1) % count };
                Outcome::None
            }
            "backspace" => {
                self.query.pop();
                self.selected = 0;
                Outcome::None
            }
            _ => {
                if let Some(text) = typed.filter(|text| !text.chars().any(char::is_control))
                    && self.query.chars().count() + text.chars().count() <= MAX_QUERY
                {
                    self.query.push_str(text);
                    self.selected = 0;
                }
                Outcome::None
            }
        }
    }
}

#[cfg(test)]
#[path = "model_picker_tests.rs"]
mod tests;
