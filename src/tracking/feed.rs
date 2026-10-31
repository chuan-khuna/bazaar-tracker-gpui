//! A `tracing` layer that hands each log record, as a row of the log panel, to the window. It sits
//! beside the layer that writes `logs/tracker.log`, so the panel and the file show the same records
//! (those at info level and above), including the ones the tracker crate logs.

use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

use super::log::{Kind, LogRow};

pub struct FeedLayer {
    rows: async_channel::Sender<LogRow>,
}

/// The layer, and the receiver of the rows it makes. Records made while nothing listens are dropped.
pub fn channel() -> (FeedLayer, async_channel::Receiver<LogRow>) {
    let (rows, receiver) = async_channel::unbounded();
    (FeedLayer { rows }, receiver)
}

impl<S: Subscriber> Layer<S> for FeedLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut text = Text::default();
        event.record(&mut text);
        let kind = match *event.metadata().level() {
            Level::ERROR => Kind::Error,
            Level::WARN => Kind::Warn,
            _ => Kind::Info,
        };
        let time_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64);
        let _ = self.rows.try_send(LogRow { time_ms, kind, text: text.finish() });
    }
}

/// Collects a record's message and its other fields as `message (name: value, ...)`.
#[derive(Default)]
struct Text {
    message: String,
    fields: Vec<String>,
}

impl Text {
    fn finish(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (_, true) => self.message,
            (true, false) => self.fields.join(", "),
            (false, false) => format!("{} ({})", self.message, self.fields.join(", ")),
        }
    }

    fn put(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = value;
        } else {
            self.fields.push(format!("{}: {value}", field.name().replace('_', " ")));
        }
    }
}

impl Visit for Text {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value.to_owned());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, value.to_string());
    }

    // The message arrives here as formatted arguments, which print as plain text.
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.put(field, format!("{value:?}"));
    }
}

#[cfg(test)]
#[path = "feed_tests.rs"]
mod tests;
