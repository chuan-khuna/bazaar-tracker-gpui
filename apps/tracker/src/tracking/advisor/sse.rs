//! Server-sent events: the reply stream's lines turned into frames, and frames into the Advisor's
//! events.
//!
//! The backend sends each event as `event: <type>` then `data: <one line of JSON>` then a blank
//! line. The parser follows the SSE rules that matter here: lines may arrive in pieces, a frame may
//! have several `data:` lines (joined with `\n`), a line starting with `:` is a comment, and a blank
//! line ends the frame. The event's name comes from `event:`, else from the JSON's `type`.

use serde_json::Value;

/// One frame: its event name (empty when it had none) and its data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub event: String,
    pub data: String,
}

/// Turns the stream's text into frames, however it is cut.
#[derive(Debug, Default)]
pub struct SseParser {
    /// A line that has not ended yet.
    partial: String,
    event: String,
    data: Vec<String>,
}

impl SseParser {
    /// Take in the next piece of the stream; the frames it finished, in order.
    pub fn feed(&mut self, chunk: &str) -> Vec<Frame> {
        self.partial.push_str(chunk);
        let mut frames = Vec::new();
        while let Some(end) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=end).collect();
            let line = line.trim_end_matches('\n').trim_end_matches('\r');
            frames.extend(self.line(line));
        }
        frames
    }

    /// The stream ended: a frame still open counts if it had data.
    pub fn finish(&mut self) -> Option<Frame> {
        let rest = std::mem::take(&mut self.partial);
        let mut frame = None;
        if !rest.is_empty() {
            frame = self.line(rest.trim_end_matches('\r'));
        }
        frame.or_else(|| self.line(""))
    }

    /// One whole line, without its line ending.
    fn line(&mut self, line: &str) -> Option<Frame> {
        if line.is_empty() {
            if self.data.is_empty() {
                self.event.clear();
                return None;
            }
            let frame = Frame { event: std::mem::take(&mut self.event), data: self.data.join("\n") };
            self.data.clear();
            return Some(frame);
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = value.to_owned(),
            "data" => self.data.push(value.to_owned()),
            // `id:` and `retry:` mean nothing to this client.
            _ => {}
        }
        None
    }
}

/// What a `delta` adds to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeltaKind {
    Text,
    Thinking,
    ToolCall,
    Other(String),
}

/// One event of a reply. The messages and stats are kept as the JSON the backend sent.
#[derive(Clone, Debug, PartialEq)]
pub enum AdvisorEvent {
    /// A message begins: the user's, an assistant's (still empty), a tool result.
    MessageStart(Value),
    /// More of the assistant message streaming: text, thinking, or a piece of a tool call's arguments.
    Delta { kind: DeltaKind, index: usize, delta: String },
    /// A message is whole, and saved.
    MessageEnd(Value),
    /// The reply's usage, after its last message.
    Stats(Value),
    /// The reply failed outside the model call.
    Error(String),
    /// Always last.
    Done,
    /// An event this app does not know: passed over.
    Unknown(String),
}

impl AdvisorEvent {
    /// The event a frame carries. Data that is not JSON is an `Unknown` event.
    pub fn from_frame(frame: &Frame) -> Self {
        let Ok(json) = serde_json::from_str::<Value>(&frame.data) else {
            return Self::Unknown(frame.event.clone());
        };
        let name = if frame.event.is_empty() { json.get("type").and_then(Value::as_str).unwrap_or_default().to_owned() } else { frame.event.clone() };
        let text = |key: &str| json.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        match name.as_str() {
            "message_start" => Self::MessageStart(json.get("message").cloned().unwrap_or(Value::Null)),
            "message_end" => Self::MessageEnd(json.get("message").cloned().unwrap_or(Value::Null)),
            "delta" => {
                let kind = match json.get("kind").and_then(Value::as_str).unwrap_or_default() {
                    "text" => DeltaKind::Text,
                    "thinking" => DeltaKind::Thinking,
                    "toolcall" => DeltaKind::ToolCall,
                    other => DeltaKind::Other(other.to_owned()),
                };
                let index = json.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                Self::Delta { kind, index, delta: text("delta") }
            }
            "stats" => Self::Stats(json),
            "error" => Self::Error(text("error")),
            "done" => Self::Done,
            _ => Self::Unknown(name),
        }
    }
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
