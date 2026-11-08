//! A Run's Conversations as the API lists them, and the text of the one open in the tab: its saved
//! messages, with a reply's text added as it streams.

use serde::Deserialize;
use serde_json::Value;

use super::sse::{AdvisorEvent, DeltaKind};

/// A Conversation: `{id, runId, title, day, hour, createdMs}`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub run_id: String,
    /// The first question on one line, cut on a word to 48 characters by the backend.
    pub title: String,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub created_ms: i64,
}

impl Conversation {
    /// `D5 H2`: the Day and Hour the Conversation began at; `D5` without the hour; nothing without the day.
    pub fn time(&self) -> Option<String> {
        time_label(self.day, self.hour)
    }

    /// Its line in the list: the title, then the Day and Hour.
    pub fn label(&self) -> String {
        match self.time() {
            Some(time) => format!("{} {time}", self.title),
            None => self.title.clone(),
        }
    }
}

/// `D5 H2`, `D5`, or nothing.
pub fn time_label(day: Option<i32>, hour: Option<i32>) -> Option<String> {
    match (day, hour) {
        (Some(day), Some(hour)) => Some(format!("D{day} H{hour}")),
        (Some(day), None) => Some(format!("D{day}")),
        _ => None,
    }
}

/// Newest first, as the list shows them.
pub fn newest_first(conversations: &mut [Conversation]) {
    conversations.sort_by_key(|conversation| std::cmp::Reverse(conversation.created_ms));
}

/// Who a line of the Conversation is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// A failed reply's error.
    Error,
}

/// One message as the tab shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub role: Role,
    pub text: String,
    /// The tools an assistant message called, by name.
    pub tools: Vec<String>,
}

impl Entry {
    fn new(role: Role, text: impl Into<String>) -> Self {
        Self { role, text: text.into(), tools: Vec::new() }
    }
}

/// The open Conversation's messages, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    pub entries: Vec<Entry>,
}

impl Transcript {
    /// From the messages the API saved (`message` of each, in order). Tool results are not shown.
    pub fn from_saved(messages: &[Value]) -> Self {
        let mut transcript = Self::default();
        for message in messages {
            transcript.entries.extend(entry_of(message));
            transcript.entries.extend(error_of(message));
        }
        transcript
    }

    /// Take in one event of a streaming reply.
    pub fn apply(&mut self, event: &AdvisorEvent) {
        match event {
            AdvisorEvent::MessageStart(message) => match role_of(message) {
                Some("user") => self.entries.extend(entry_of(message)),
                Some("assistant") => self.entries.push(Entry::new(Role::Assistant, "")),
                _ => {}
            },
            AdvisorEvent::Delta { kind: DeltaKind::Text, delta, .. } => {
                if let Some(entry) = self.entries.last_mut().filter(|entry| entry.role == Role::Assistant) {
                    entry.text.push_str(delta);
                }
            }
            // The whole message replaces what streamed.
            AdvisorEvent::MessageEnd(message) if role_of(message) == Some("assistant") => {
                let whole = entry_of(message);
                match self.entries.last_mut().filter(|entry| entry.role == Role::Assistant) {
                    Some(entry) => match whole {
                        Some(whole) => *entry = whole,
                        None => {
                            self.entries.pop();
                        }
                    },
                    None => self.entries.extend(whole),
                }
                self.entries.extend(error_of(message));
            }
            AdvisorEvent::Error(error) => self.entries.push(Entry::new(Role::Error, error.clone())),
            _ => {}
        }
    }
}

/// A failed model call's error, as an entry of its own.
fn error_of(message: &Value) -> Option<Entry> {
    let error = message.get("errorMessage").and_then(Value::as_str).filter(|error| !error.is_empty())?;
    Some(Entry::new(Role::Error, error))
}

fn role_of(message: &Value) -> Option<&str> {
    message.get("role").and_then(Value::as_str)
}

/// A user or assistant message as an entry: its text blocks joined, and the tools it called. An
/// assistant message with neither (an empty stopped one) is none, as is a tool result.
fn entry_of(message: &Value) -> Option<Entry> {
    let role = match role_of(message)? {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        _ => return None,
    };
    let (text, tools) = match message.get("content") {
        Some(Value::String(text)) => (text.clone(), Vec::new()),
        Some(Value::Array(blocks)) => {
            let text: Vec<&str> = blocks.iter().filter(|block| block.get("type").and_then(Value::as_str) == Some("text")).filter_map(|block| block.get("text").and_then(Value::as_str)).collect();
            let tools = blocks.iter().filter(|block| block.get("type").and_then(Value::as_str) == Some("toolCall")).filter_map(|block| block.get("name").and_then(Value::as_str)).map(str::to_owned).collect();
            (text.join(""), tools)
        }
        _ => (String::new(), Vec::new()),
    };
    if role == Role::Assistant && text.is_empty() && tools.is_empty() {
        return None;
    }
    Some(Entry { role, text, tools })
}

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;
