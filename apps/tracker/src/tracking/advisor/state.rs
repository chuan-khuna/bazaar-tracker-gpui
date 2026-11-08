//! What the Advisor tab shows, and what it asks the backend for next, with no drawing and no I/O.
//!
//! Each change the player makes or each [`Update`](super::client::Update) the client sends back
//! goes through a method here, which changes the state and returns the [`Request`]s to run. The
//! tab follows one Run (the selected one), lists its Conversations newest first, and shows one of
//! them, or none until the first message makes one.

use std::collections::HashSet;
use std::time::Instant;

use serde_json::Value;

use super::ask_user::Draft;
use super::client::{ClientError, ModelList, ModelOption, Saved};
use super::conversation::{Conversation, newest_first};
use super::recommendation::Hover;
use super::reply::Transcript;
use super::sse::AdvisorEvent;

/// What the tab says when nothing answers, followed by [`SERVE_COMMAND`] as copyable inline code.
pub const NOT_RUNNING: &str = "The Advisor isn't running. Start it with";

/// The command that starts the backend: drawn after [`NOT_RUNNING`], and what its copy button copies.
pub const SERVE_COMMAND: &str = "just advisor serve";

/// What the model picker shows while no model is chosen and the backend hasn't said its default.
pub const BACKEND_MODEL: &str = "Advisor's default model";

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
    /// Make a Conversation for its first question, then send it there (answered by `model`).
    Create { run_id: String, text: String, situation: Value, model: Option<String> },
    Messages { conversation_id: String },
    /// `model`: the one chosen in the picker; `None` lets the backend use its `ADVISOR_MODEL`.
    Send { conversation_id: String, text: String, situation: Value, model: Option<String> },
    /// `GET /api/models`, for the model picker.
    Models,
    Delete { conversation_id: String },
    /// Close the Conversation's reply stream, which cancels the reply.
    Stop { conversation_id: String },
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
    /// The Conversation whose reply was stopped and whose stream hasn't ended yet: what still
    /// comes of it is passed over.
    stopped: Option<String>,
    /// A new Conversation is being made for the first message.
    creating: bool,
    /// Stop was pressed while the Conversation was being made: the message isn't sent to it.
    cancel_create: bool,
    /// The waiting `ask_user` card's picks and words.
    draft: Draft,
    /// The thinking blocks, tool lines and recommendation cards' runs opened, by their key.
    open: HashSet<String>,
    /// The last message sent, put back in the box if it never reached the backend.
    last_sent: Option<String>,
    /// The Conversation whose ✕ was clicked once: a second click deletes it.
    deleting: Option<String>,
    backend: Backend,
    /// The last error the backend answered with, other than being unreachable or unavailable.
    error: Option<String>,
    /// What is typed in the box.
    pub input: String,
    /// Which recommendation card count's popup of source runs is showing.
    pub hover: Hover,
    /// The model chosen in the picker, sent with every message; `None` is the backend's own
    /// `ADVISOR_MODEL`. Starts on the saved default, and is kept when another Run is shown.
    model: Option<String>,
    /// The last list `GET /api/models` gave, kept when another Run is shown.
    models: Option<ModelList>,
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

    /// Whether Send is Stop: a message of this app's is on its way to the shown Conversation, and
    /// hasn't been stopped yet.
    pub fn can_stop(&self) -> bool {
        (self.creating && !self.cancel_create) || (self.streaming.is_some() && self.streaming == self.selected && self.stopped != self.streaming)
    }

    /// Whether a thinking block, tool line or recommendation card's run with this key is open.
    pub fn is_open(&self, key: &str) -> bool {
        self.open.contains(key)
    }

    /// Open or close a thinking block, tool line or recommendation card's run.
    pub fn toggle(&mut self, key: &str) {
        if !self.open.remove(key) {
            self.open.insert(key.to_owned());
        }
    }

    /// The waiting `ask_user` card's picks and words (for the card it answers: see `Draft::call_id`).
    pub fn draft(&self) -> &Draft {
        &self.draft
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

    /// The command drawn after the status, with a copy button: [`SERVE_COMMAND`] while the
    /// backend isn't running.
    pub fn status_command(&self) -> Option<&'static str> {
        (self.backend == Backend::Unreachable).then_some(SERVE_COMMAND)
    }

    /// The model chosen in the picker; `None` is the backend's own.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Choose the model for the messages sent from now on (`None`: the backend's own). Blank is
    /// `None`.
    pub fn choose_model(&mut self, model: Option<&str>) {
        self.model = model.map(str::trim).filter(|model| !model.is_empty()).map(str::to_owned);
    }

    /// What the picker shows: the chosen model, else the backend's default once `GET /api/models`
    /// has said it, else [`BACKEND_MODEL`]. Works without the backend.
    pub fn model_label(&self) -> String {
        match (&self.model, &self.models) {
            (Some(model), _) => model.clone(),
            (None, Some(list)) if !list.default.is_empty() => list.default.clone(),
            _ => BACKEND_MODEL.to_owned(),
        }
    }

    /// The models the picker lists (empty until `GET /api/models` answers).
    pub fn models(&self) -> &[ModelOption] {
        self.models.as_ref().map_or(&[], |list| list.models.as_slice())
    }

    /// The backend's own default model, once it has said it.
    pub fn backend_model(&self) -> Option<&str> {
        self.models.as_ref().map(|list| list.default.as_str()).filter(|model| !model.is_empty())
    }

    /// The picker opens: ask for the list again (the backend caches it).
    pub fn open_picker(&self) -> Vec<Request> {
        vec![Request::Models]
    }

    /// `GET /api/models` answered. A failure keeps the last list; the picker still shows the chosen model.
    pub fn on_models(&mut self, result: Result<ModelList, ClientError>) {
        if let Ok(list) = result {
            self.models = Some(list);
        }
    }

    /// Show `run_id`'s Conversations. A Run not shown yet starts at its newest Conversation; so
    /// does any Run when `newest` (a chat icon). The list is read again when it hasn't been, when
    /// the backend wasn't usable last time (so opening the tab tries again), or for `newest`.
    pub fn open(&mut self, run_id: &str, newest: bool) -> Vec<Request> {
        if self.run_id.as_deref() != Some(run_id) {
            let input = std::mem::take(&mut self.input);
            *self = Self { run_id: Some(run_id.to_owned()), backend: self.backend.clone(), input, model: self.model.take(), models: self.models.take(), ..Self::default() };
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
        self.draft = Draft::default();
        self.open.clear();
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
                    self.transcript = Transcript::from_saved(&saved.messages);
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
        let requests = self.send_text(text, situation);
        if !requests.is_empty() {
            self.input.clear();
        }
        requests
    }

    /// A `follow_ups` button: send its question as if typed.
    pub fn follow_up(&mut self, question: &str, situation: Value) -> Vec<Request> {
        self.send_text(question.trim().to_owned(), situation)
    }

    /// A click on a choice of the waiting `ask_user` card.
    pub fn ask_pick(&mut self, call_id: &str, option_id: &str) {
        if let Some(ask) = self.waiting(call_id) {
            self.draft.pick(&ask, option_id);
        }
    }

    /// The words in the waiting card's text box.
    pub fn ask_text(&self) -> &str {
        &self.draft.typed
    }

    /// Change the words in the waiting card's text box.
    pub fn set_ask_text(&mut self, text: String) {
        if let Some((call_id, ask)) = self.transcript.waiting_question() {
            if self.draft.call_id != call_id {
                self.draft = Draft { call_id, ..Draft::default() };
            }
            self.draft.typed(&ask, text);
        }
    }

    /// The card's Send: its answer in tau-app's format, sent as an ordinary message.
    pub fn answer(&mut self, call_id: &str, situation: Value) -> Vec<Request> {
        let Some(ask) = self.waiting(call_id) else { return Vec::new() };
        let text = ask.answer(&self.draft.picked, &self.draft.typed);
        let requests = self.send_text(text, situation);
        if !requests.is_empty() {
            self.draft = Draft::default();
        }
        requests
    }

    /// The waiting question if it is `call_id`'s, with the draft made its own.
    fn waiting(&mut self, call_id: &str) -> Option<super::ask_user::AskUser> {
        let (waiting, ask) = self.transcript.waiting_question().filter(|(waiting, _)| waiting == call_id)?;
        if self.draft.call_id != waiting {
            self.draft = Draft { call_id: waiting, ..Draft::default() };
        }
        Some(ask)
    }

    fn send_text(&mut self, text: String, situation: Value) -> Vec<Request> {
        let Some(run_id) = self.run_id.clone() else { return Vec::new() };
        if text.is_empty() || self.busy() || self.running_elsewhere {
            return Vec::new();
        }
        self.error = None;
        self.deleting = None;
        self.last_sent = Some(text.clone());
        match self.selected.clone() {
            Some(conversation_id) => {
                self.streaming = Some(conversation_id.clone());
                vec![Request::Send { conversation_id, text, situation, model: self.model.clone() }]
            }
            None => {
                self.creating = true;
                vec![Request::Create { run_id, text, situation, model: self.model.clone() }]
            }
        }
    }

    /// The Conversation made for the first message: shown, and the message sent to it, answered by
    /// the model chosen when it was sent.
    pub fn on_created(&mut self, run_id: &str, text: String, situation: Value, model: Option<String>, result: Result<Conversation, ClientError>) -> Vec<Request> {
        self.creating = false;
        let cancelled = std::mem::take(&mut self.cancel_create);
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
        self.draft = Draft::default();
        self.open.clear();
        if cancelled {
            // Stopped before it was sent: the empty Conversation is shown, the message goes back.
            self.give_back();
            return Vec::new();
        }
        self.streaming = Some(conversation_id.clone());
        vec![Request::Send { conversation_id, text, situation, model }]
    }

    /// One event of the reply streaming to this Conversation, which came at `at`. A stopped
    /// reply's last events are passed over.
    pub fn on_event(&mut self, conversation_id: &str, event: &AdvisorEvent, at: Instant) {
        if self.selected.as_deref() == Some(conversation_id) && self.stopped.as_deref() != Some(conversation_id) {
            self.transcript.apply(event, at);
        }
    }

    /// Stop: close the stream of the reply on its way to the shown Conversation, keeping what
    /// arrived, marked stopped. While the Conversation is still being made, the message isn't sent.
    pub fn stop(&mut self, at: Instant) -> Vec<Request> {
        if self.creating {
            self.cancel_create = true;
            return Vec::new();
        }
        let Some(conversation_id) = self.streaming.clone().filter(|_| self.can_stop()) else { return Vec::new() };
        self.transcript.stop(at);
        self.stopped = Some(conversation_id.clone());
        vec![Request::Stop { conversation_id }]
    }

    /// The reply's stream ended at `at`. A message that never reached the backend goes back in
    /// the box; a stream that broke off fails the reply.
    pub fn on_ended(&mut self, conversation_id: &str, result: Result<(), ClientError>, at: Instant) {
        if self.streaming.as_deref() == Some(conversation_id) {
            self.streaming = None;
        }
        let stopped = self.stopped.as_deref() == Some(conversation_id);
        if stopped {
            self.stopped = None;
        }
        let shown = self.selected.as_deref() == Some(conversation_id) && !stopped;
        match result {
            Ok(()) => {
                self.ready();
                self.last_sent = None;
                if shown {
                    self.transcript.finish(at);
                }
            }
            Err(error) => {
                if matches!(error, ClientError::Unreachable | ClientError::Status { .. }) {
                    self.give_back();
                    self.failed(&error);
                } else if shown && self.transcript.streaming() {
                    // Shown on the reply itself, not above the messages.
                    self.transcript.fail(error.to_string(), at);
                    self.ready();
                } else {
                    self.failed(&error);
                }
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
            *self = Self { backend: self.backend.clone(), input, model: self.model.take(), models: self.models.take(), ..Self::default() };
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
