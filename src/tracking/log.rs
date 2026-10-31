//! The rows of the log panel, and the buffer that keeps the latest of them.

use std::collections::VecDeque;

/// How many rows the log keeps. Older rows drop off as newer ones arrive.
pub const CAPACITY: usize = 2000;

/// What a row is about. The panel colours errors and warnings so that they stand out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Info,
    Warn,
    Error,
    /// Something that happened in a Run.
    Event,
}

impl Kind {
    /// The word the panel shows beside the time.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Info => "info",
            Kind::Warn => "warn",
            Kind::Error => "error",
            Kind::Event => "event",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRow {
    /// When it happened, in milliseconds since the Unix epoch.
    pub time_ms: i64,
    pub kind: Kind,
    /// A plain sentence.
    pub text: String,
}

/// The latest [`CAPACITY`] rows, oldest first.
#[derive(Debug, Default)]
pub struct LogBuffer {
    rows: VecDeque<LogRow>,
    capacity: usize,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self::with_capacity(CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self { rows: VecDeque::with_capacity(capacity.min(CAPACITY)), capacity }
    }

    pub fn push(&mut self, row: LogRow) {
        if self.capacity == 0 {
            return;
        }
        while self.rows.len() >= self.capacity {
            self.rows.pop_front();
        }
        self.rows.push_back(row);
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = &LogRow> {
        self.rows.iter()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
