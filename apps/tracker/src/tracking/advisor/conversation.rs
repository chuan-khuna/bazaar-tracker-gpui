//! A Run's Conversations as the API lists them, and their titles.

use serde::Deserialize;

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

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;
