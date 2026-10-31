//! The loose matching behind the theme picker and the command palette.

/// Whether the letters of `query` appear in `text` in order, but not necessarily next to each other,
/// so "cmo" matches "Catppuccin Mocha". Case, accents and spaces in the query are ignored.
pub fn matches(query: &str, text: &str) -> bool {
    let text = fold(text);
    let mut letters = text.chars();
    fold(query).chars().filter(|c| !c.is_whitespace()).all(|q| letters.any(|t| t == q))
}

fn fold(text: &str) -> String {
    text.to_lowercase().replace('é', "e")
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
