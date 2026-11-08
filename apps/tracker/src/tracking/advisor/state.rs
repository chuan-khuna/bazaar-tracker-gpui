//! What the Advisor tab shows, and what it asks the backend for next, with no drawing and no I/O.
//!
//! Each change the player makes or each [`Update`](super::client::Update) the client sends back
//! goes through a method here, which changes the state and returns the [`Request`]s to run. The
//! tab follows one Run (the selected one), lists its Conversations newest first, and shows one of
//! them, or none until the first message makes one.

use serde_json::Value;

use super::client::{ClientError, Saved};
use super::conversation::{Conversation, Transcript, newest_first};
use super::sse::AdvisorEvent;

/// What the tab says when nothing answers.
pub const NOT_RUNNING: &str = "The Advisor isn't running. Start it with `just advisor serve`.";

/// The detail island's tabs. The open one is kept when another Run is selected, and isn't saved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Run,
    Advisor,
}

/// A chat icon: on a sidebar Run's row, or on the Main island beside the pop-out button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatIcon<'a> {
    Row(&'a str),
    Main,
}

/// The Run a chat icon opens: a row's own Run, or for the Main island's the Live run (with none
/// it is greyed out and opens nothing).
pub fn icon_run<'a>(icon: ChatIcon<'a>, live: Option<&'a str>) -> Option<&'a str> {
    match icon {
        ChatIcon::Row(run_id) => Some(run_id),
        ChatIcon::Main => live,
    }
}

/// What the client is asked to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    List { run_id: String },
    /// Make a Conversation for its first question, then send it there.
    Create { run_id: String, text: String, situation: Value },
    Messages { conversation_id: String },
    Send { conversation_id: String, text: String, situation: Value },
    Delete { conversation_id: String },
}

/// Whether the backend could be used, as last heard.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// Not asked yet.
    #[default]
    Unknown,
    Ready,
    /// Nothing answered.
    Unreachable,
    /// It answered 503: it is running but can't use `history.db`, for this reason.
    Unavailable(String),
}

#[derive(Debug, Default)]
pub struct AdvisorState {
    /// The Run whose Conversations are listed.
    run_id: Option<String>,
    /// Newest first.
    conversations: Vec<Conversation>,
    /// Whether `conversations` has been read for `run_id`.
    listed: bool,
    /// The Conversation shown; none is a new one, made by the first message.
    selected: Option<String>,
    /// Show the newest Conversation once the list comes.
    want_newest: bool,
    /// The shown Conversation's messages.
    transcript: Transcript,
    /// Whether the backend says the shown Conversation has a reply running (sent from elsewhere).
    running_elsewhere: bool,
    /// The Conversation whose reply this app is streaming.
    streaming: Option<String>,
    /// A new Conversation is being made for the first message.
    creating: bool,
    /// The last message sent, put back in the box if it never reached the backend.
    last_sent: Option<String>,
    /// The Conversation whose ✕ was clicked once: a second click deletes it.
    deleting: Option<String>,
    backend: Backend,
    /// The last error the backend answered with, other than being unreachable or unavailable.
    error: Option<String>,
    /// What is typed in the box.
    pub input: String,
}

impl AdvisorState {
    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }

    pub fn conversations(&self) -> &[Conversation] {
        &self.conversations
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub fn deleting(&self) -> Option<&str> {
        self.deleting.as_deref()
    }

    /// Whether a message is on its way: a Conversation being made, or a reply streaming.
    pub fn busy(&self) -> bool {
        self.creating || self.streaming.is_some()
    }

    /// Whether the shown Conversation has a reply running, here or elsewhere.
    pub fn replying(&self) -> bool {
        self.creating || (self.streaming.is_some() && self.streaming == self.selected) || self.running_elsewhere
    }

    /// What the tab says above the messages, if anything: the backend isn't running, can't be
    /// used, or answered with an error.
    pub fn status(&self) -> Option<String> {
        match &self.backend {
            Backend::Unreachable => Some(NOT_RUNNING.to_owned()),
            Backend::Unavailable(why) => Some(why.clone()),
            Backend::Unknown | Backend::Ready => self.error.clone(),
        }
    }

    /// Show `run_id`'s Conversations. A Run not shown yet starts at its newest Conversation; so
    /// does any Run when `newest` (a chat icon). The list is read again when it hasn't been, when
    /// the backend wasn't usable last time (so opening the tab tries again), or for `newest`.
    pub fn open(&mut self, run_id: &str, newest: bool) -> Vec<Request> {
        if self.run_id.as_deref() != Some(run_id) {
            let input = std::mem::take(&mut self.input);
            *self = Self { run_id: Some(run_id.to_owned()), backend: self.backend.clone(), input, ..Self::default() };
            self.want_newest = true;
        } else if newest {
            self.want_newest = true;
        } else if self.listed && self.backend == Backend::Ready {
            return Vec::new();
        }
        vec![Request::List { run_id: run_id.to_owned() }]
    }

    /// The Run's Conversations came. The newest is shown if wanted, or if the shown one is gone.
    pub fn on_conversations(&mut self, run_id: &str, result: Result<Vec<Conversation>, ClientError>) -> Vec<Request> {
        if self.run_id.as_deref() != Some(run_id) {
            return Vec::new();
        }
        let mut list = match result {
            Ok(list) => list,
            Err(error) => {
                self.failed(&error);
                return Vec::new();
            }
        };
        self.ready();
        newest_first(&mut list);
        self.conversations = list;
        self.listed = true;
        let gone = self.selected.as_ref().is_some_and(|id| !self.conversations.iter().any(|conversation| &conversation.id == id));
        if !(std::mem::take(&mut self.want_newest) || gone) {
            return Vec::new();
        }
        // While a new Conversation is being made, it will be shown once it is.
        if self.creating {
            return Vec::new();
        }
        let newest = self.conversations.first().map(|conversation| conversation.id.clone());
        self.show(newest)
    }

    /// Show the Conversation with this id.
    pub fn select(&mut self, id: &str) -> Vec<Request> {
        if !self.conversations.iter().any(|conversation| conversation.id == id) {
            return Vec::new();
        }
        self.deleting = None;
        self.show(Some(id.to_owned()))
    }

    /// `+`: show no Conversation, so the next message makes a new one.
    pub fn new_conversation(&mut self) {
        self.deleting = None;
        self.show(None);
    }

    fn show(&mut self, id: Option<String>) -> Vec<Request> {
        self.transcript = Transcript::default();
        self.running_elsewhere = false;
        self.selected = id.clone();
        id.map(|conversation_id| Request::Messages { conversation_id }).into_iter().collect()
    }

    pub fn on_messages(&mut self, conversation_id: &str, result: Result<Saved, ClientError>) {
        if self.selected.as_deref() != Some(conversation_id) {
            return;
        }
        match result {
            Ok(saved) => {
                self.ready();
                // A reply this app is streaming adds its own messages as they come.
                if self.streaming.as_deref() != Some(conversation_id) {
                    self.transcript = Transcript::from_saved(&saved.messages.iter().map(|saved| saved.message.clone()).collect::<Vec<_>>());
                    self.running_elsewhere = saved.running;
                }
            }
            Err(error) => self.failed(&error),
        }
    }

    /// Enter: send what is typed, with `situation`, to the shown Conversation, or to a new one made
    /// for it. Nothing is sent while a message is on its way, without a Run, or with nothing typed.
    pub fn send(&mut self, situation: Value) -> Vec<Request> {
        let text = self.input.trim().to_owned();
        let Some(run_id) = self.run_id.clone() else { return Vec::new() };
        if text.is_empty() || self.busy() || self.running_elsewhere {
            return Vec::new();
        }
        self.input.clear();
        self.error = None;
        self.deleting = None;
        self.last_sent = Some(text.clone());
        match self.selected.clone() {
            Some(conversation_id) => {
                self.streaming = Some(conversation_id.clone());
                vec![Request::Send { conversation_id, text, situation }]
            }
            None => {
                self.creating = true;
                vec![Request::Create { run_id, text, situation }]
            }
        }
    }

    /// The Conversation made for the first message: shown, and the message sent to it.
    pub fn on_created(&mut self, run_id: &str, text: String, situation: Value, result: Result<Conversation, ClientError>) -> Vec<Request> {
        self.creating = false;
        let conversation = match result {
            Ok(conversation) => conversation,
            Err(error) => {
                self.failed(&error);
                self.give_back();
                return Vec::new();
            }
        };
        self.ready();
        if self.run_id.as_deref() != Some(run_id) {
            // The player moved to another Run meanwhile: the message isn't sent.
            return Vec::new();
        }
        let conversation_id = conversation.id.clone();
        self.conversations.retain(|known| known.id != conversation_id);
        self.conversations.insert(0, conversation);
        self.selected = Some(conversation_id.clone());
        self.transcript = Transcript::default();
        self.streaming = Some(conversation_id.clone());
        vec![Request::Send { conversation_id, text, situation }]
    }

    /// One event of the reply streaming to this Conversation.
    pub fn on_event(&mut self, conversation_id: &str, event: &AdvisorEvent) {
        if self.selected.as_deref() == Some(conversation_id) {
            self.transcript.apply(event);
        }
    }

    /// The reply's stream ended. A message that never reached the backend goes back in the box.
    pub fn on_ended(&mut self, conversation_id: &str, result: Result<(), ClientError>) {
        if self.streaming.as_deref() == Some(conversation_id) {
            self.streaming = None;
        }
        match result {
            Ok(()) => {
                self.ready();
                self.last_sent = None;
            }
            Err(error) => {
                if matches!(error, ClientError::Unreachable | ClientError::Status { .. }) {
                    self.give_back();
                }
                self.failed(&error);
            }
        }
    }

    /// ✕ on a Conversation: the first click asks, the second deletes it.
    pub fn delete(&mut self, id: &str) -> Vec<Request> {
        if self.deleting.as_deref() == Some(id) {
            self.deleting = None;
            vec![Request::Delete { conversation_id: id.to_owned() }]
        } else {
            self.deleting = Some(id.to_owned());
            Vec::new()
        }
    }

    pub fn on_deleted(&mut self, conversation_id: &str, result: Result<(), ClientError>) -> Vec<Request> {
        if let Err(error) = result {
            self.failed(&error);
            return Vec::new();
        }
        self.ready();
        self.conversations.retain(|conversation| conversation.id != conversation_id);
        if self.selected.as_deref() == Some(conversation_id) {
            let newest = self.conversations.first().map(|conversation| conversation.id.clone());
            return self.show(newest);
        }
        Vec::new()
    }

    /// The Run got its real id: its Conversations went with it.
    pub fn rebound(&mut self, old: &str, new: &str) {
        if self.run_id.as_deref() == Some(old) {
            self.run_id = Some(new.to_owned());
            for conversation in &mut self.conversations {
                conversation.run_id = new.to_owned();
            }
        }
    }

    /// The Run was deleted (or every Run cleared): nothing of it is shown.
    pub fn forget(&mut self, run_id: Option<&str>) {
        if run_id.is_none() || self.run_id.as_deref() == run_id {
            let input = std::mem::take(&mut self.input);
            *self = Self { backend: self.backend.clone(), input, ..Self::default() };
        }
    }

    fn ready(&mut self) {
        self.backend = Backend::Ready;
    }

    fn failed(&mut self, error: &ClientError) {
        match error {
            ClientError::Unreachable => self.backend = Backend::Unreachable,
            ClientError::Status { status: 503, detail } => self.backend = Backend::Unavailable(detail.clone()),
            other => {
                self.backend = Backend::Ready;
                self.error = Some(other.to_string());
            }
        }
    }

    /// Put the last message back in the box, unless something new was typed.
    fn give_back(&mut self) {
        if let Some(text) = self.last_sent.take()
            && self.input.trim().is_empty()
        {
            self.input = text;
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
