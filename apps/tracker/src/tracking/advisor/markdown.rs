//! Light markdown for the Advisor's text: bold, italics, inline code, and bullet and numbered
//! lists. Anything else (headings, tables, links, quotes, fenced code) stays plain text.
//!
//! Text is split into lines, each with its kind and its text with the ranges to style, the shape
//! gpui's `StyledText` draws. A marker with no partner (as while a reply streams in) stays as typed.

use std::ops::Range;

/// How a range of a line is styled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Emphasis {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

impl Emphasis {
    const BOLD: Self = Self { bold: true, italic: false, code: false };
    const ITALIC: Self = Self { bold: false, italic: true, code: false };
    const CODE: Self = Self { bold: false, italic: false, code: true };

    fn with(self, other: Self) -> Self {
        Self { bold: self.bold || other.bold, italic: self.italic || other.italic, code: self.code || other.code }
    }

    fn is_plain(self) -> bool {
        self == Self::default()
    }
}

/// What a line is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// Plain text, or an empty line (a gap).
    Plain,
    /// `- `, `* ` or `+ `; `depth` is the indent in steps of two spaces.
    Bullet { depth: usize },
    /// `1. ` or `1) `, keeping its number.
    Numbered { depth: usize, number: String },
}

/// One line: its kind, its text without the list marker, and the styled ranges of that text
/// (byte ranges, in order, not overlapping; the rest is plain).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: LineKind,
    pub text: String,
    pub runs: Vec<(Range<usize>, Emphasis)>,
}

/// The lines of `text`. Lines inside a ``` fence are kept as plain text, fences included.
pub fn parse(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut fenced = false;
    for raw in text.split('\n') {
        let raw = raw.trim_end_matches('\r');
        if raw.trim_start().starts_with("```") {
            fenced = !fenced;
            lines.push(plain(raw));
            continue;
        }
        if fenced {
            lines.push(plain(raw));
            continue;
        }
        let (kind, rest) = list_marker(raw);
        let (text, runs) = inline(rest);
        lines.push(Line { kind, text, runs });
    }
    lines
}

fn plain(text: &str) -> Line {
    Line { kind: LineKind::Plain, text: text.to_owned(), runs: Vec::new() }
}

/// The line's list marker, if any, and the text after it.
fn list_marker(line: &str) -> (LineKind, &str) {
    let rest = line.trim_start_matches(' ');
    let depth = (line.len() - rest.len()) / 2;
    for bullet in ["- ", "* ", "+ "] {
        if let Some(after) = rest.strip_prefix(bullet) {
            return (LineKind::Bullet { depth }, after.trim_start());
        }
    }
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=3).contains(&digits) {
        let after = &rest[digits..];
        if let Some(after) = after.strip_prefix(". ").or_else(|| after.strip_prefix(") ")) {
            return (LineKind::Numbered { depth, number: rest[..digits].to_owned() }, after.trim_start());
        }
    }
    (LineKind::Plain, line)
}

/// A line's text with its markers taken out, and the styled ranges.
pub fn inline(line: &str) -> (String, Vec<(Range<usize>, Emphasis)>) {
    let mut pieces = Vec::new();
    pieces_of(line, Emphasis::default(), &mut pieces);
    let mut text = String::new();
    let mut runs: Vec<(Range<usize>, Emphasis)> = Vec::new();
    for (piece, emphasis) in pieces {
        if piece.is_empty() {
            continue;
        }
        let start = text.len();
        text.push_str(&piece);
        if emphasis.is_plain() {
            continue;
        }
        match runs.last_mut() {
            Some((range, last)) if range.end == start && *last == emphasis => range.end = text.len(),
            _ => runs.push((start..text.len(), emphasis)),
        }
    }
    (text, runs)
}

/// Split `text` into pieces with their emphasis, `outer` added to each.
fn pieces_of(text: &str, outer: Emphasis, pieces: &mut Vec<(String, Emphasis)>) {
    let mut plain = String::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let c = rest.chars().next().unwrap_or_default();
        // `\*` is a literal star (and so for the other markers).
        if c == '\\'
            && let Some(next) = rest[1..].chars().next()
            && matches!(next, '*' | '_' | '`' | '\\')
        {
            plain.push(next);
            i += 1 + next.len_utf8();
            continue;
        }
        if c == '`' {
            let ticks = rest.bytes().take_while(|b| *b == b'`').count();
            let fence = &rest[..ticks];
            if let Some(end) = rest[ticks..].find(fence)
                && end > 0
            {
                flush(&mut plain, outer, pieces);
                let code = &rest[ticks..ticks + end];
                // One space either side of the code is padding.
                let code = if code.len() > 2 && code.starts_with(' ') && code.ends_with(' ') { &code[1..code.len() - 1] } else { code };
                pieces.push((code.to_owned(), outer.with(Emphasis::CODE)));
                i += ticks + end + ticks;
                continue;
            }
            plain.push_str(fence);
            i += ticks;
            continue;
        }
        if c == '*' || c == '_' {
            let prev = text[..i].chars().next_back();
            let double = rest.len() >= 2 && rest.as_bytes()[1] == c as u8;
            let marker = if double { &rest[..2] } else { &rest[..1] };
            if let Some(end) = closing(text, i, marker, prev) {
                flush(&mut plain, outer, pieces);
                let inner = &text[i + marker.len()..end];
                let emphasis = if double { Emphasis::BOLD } else { Emphasis::ITALIC };
                pieces_of(inner, outer.with(emphasis), pieces);
                i = end + marker.len();
                continue;
            }
            plain.push_str(marker);
            i += marker.len();
            continue;
        }
        plain.push(c);
        i += c.len_utf8();
    }
    flush(&mut plain, outer, pieces);
}

fn flush(plain: &mut String, outer: Emphasis, pieces: &mut Vec<(String, Emphasis)>) {
    if !plain.is_empty() {
        pieces.push((std::mem::take(plain), outer));
    }
}

/// Where the marker opening at `open` is closed, if it can open there: the text inside must not
/// start or end with a space, and `_` only opens and closes at a word's edge (so `rate_options`
/// stays as it is).
fn closing(text: &str, open: usize, marker: &str, prev: Option<char>) -> Option<usize> {
    let after = open + marker.len();
    let first = text[after..].chars().next()?;
    if first.is_whitespace() {
        return None;
    }
    let underscore = marker.starts_with('_');
    if underscore && prev.is_some_and(char::is_alphanumeric) {
        return None;
    }
    let mut from = after;
    while let Some(found) = text[from..].find(marker) {
        let end = from + found;
        let before = text[..end].chars().next_back();
        let next = text[end + marker.len()..].chars().next();
        // A single `*` right before another is part of a `**`, not a closing one.
        let doubled = marker.len() == 1 && next == Some(marker.chars().next().unwrap_or_default());
        let edge_ok = !underscore || !next.is_some_and(char::is_alphanumeric);
        if end > after && before.is_some_and(|before| !before.is_whitespace()) && !doubled && edge_ok {
            return Some(end);
        }
        from = end + marker.len().max(if doubled { 2 } else { 1 });
    }
    None
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
