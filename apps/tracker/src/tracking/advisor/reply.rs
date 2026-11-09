//! The open Conversation as the tab draws it: the player's questions and the Advisor's replies,
//! each reply in full (thinking, tool calls with their results, text, `suggest`, `ask_user`,
//! `follow_ups`, usage, and how it ended), built from saved messages or from a reply's events as
//! they stream. Plain data: the drawing is `ui/advisor.rs`.
//!
//! A reply is every assistant message (one per model turn, a **step**) and tool result after a
//! question, up to the next question. A step's parts are its content blocks, by their index, as in
//! tau-app's chat (`useTauChat.ts`, `Message.tsx`).

use std::time::{Duration, Instant};

use serde_json::Value;

use super::ask_user::AskUser;
use super::recommendation::Recommendation;
use super::client::SavedMessage;
use super::sse::{AdvisorEvent, DeltaKind};
use super::usage::Stats;

/// The result the backend saves for a tool call cut off by Stop, and what the tab shows for one.
pub const INTERRUPTED: &str = "Tool call interrupted by user";

/// The open Conversation, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transcript {
    pub turns: Vec<Turn>,
    /// The player's message has started streaming and not ended.
    user_open: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Turn {
    /// The player's message.
    User(Question),
    Reply(Reply),
}

/// The player's message, and the situation sent with it (its Context row).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Question {
    pub text: String,
    /// `None`: none was sent, or not known yet (a streamed question gets it from the tab's state).
    pub situation: Option<Value>,
}

impl From<&str> for Question {
    fn from(text: &str) -> Self {
        Self { text: text.to_owned(), situation: None }
    }
}

/// How a reply stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    Streaming,
    Done,
    /// Cut short by Stop: what arrived is kept.
    Stopped,
    /// The model call or the reply failed, with why.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    /// One per assistant message, each its content blocks by index.
    pub steps: Vec<Vec<Part>>,
    pub stats: Option<Stats>,
    pub end: End,
    /// The last step is still streaming (its `message_start` came, its `message_end` not yet).
    open: bool,
}

impl Reply {
    fn new() -> Self {
        Self { steps: Vec::new(), stats: None, end: End::Streaming, open: false }
    }
}

/// One content block of an assistant message.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Text(String),
    Thinking(Thinking),
    Tool(ToolCall),
}

/// The model's thinking, and when it streamed (unknown for a saved one).
#[derive(Clone, Debug, PartialEq)]
pub struct Thinking {
    pub text: String,
    pub started: Option<Instant>,
    pub ended: Option<Instant>,
}

impl Thinking {
    /// `Thinking… 3s` while it streams, `Thought for 3s` after, `Thought` when the time isn't known.
    pub fn label(&self, now: Instant) -> String {
        match (self.started, self.ended) {
            (Some(started), None) => format!("Thinking… {}s", now.saturating_duration_since(started).as_secs()),
            (Some(started), Some(ended)) => format!("Thought for {}s", whole_seconds(ended.saturating_duration_since(started))),
            _ => "Thought".to_owned(),
        }
    }

    pub fn streaming(&self) -> bool {
        self.started.is_some() && self.ended.is_none()
    }
}

/// At least 1, rounded.
fn whole_seconds(duration: Duration) -> u64 {
    (duration.as_secs_f64().round() as u64).max(1)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    /// Empty while its arguments stream: the id and name come with the message's end.
    pub id: String,
    pub name: String,
    pub arguments: Value,
    /// The arguments' JSON as it streams; none once the message is whole.
    pub partial: Option<String>,
    pub result: Option<ToolResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolResult {
    pub is_error: bool,
    /// The whole result (`details`), else the text the model read.
    pub output: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    /// Its arguments are streaming, or it has no result yet: a spinner.
    Running,
    /// ✓
    Done,
    /// ✕
    Failed,
}

impl ToolCall {
    pub fn status(&self) -> ToolStatus {
        match &self.result {
            None => ToolStatus::Running,
            Some(result) if result.is_error => ToolStatus::Failed,
            Some(_) => ToolStatus::Done,
        }
    }

    /// The arguments as the line shows them when open: pretty JSON, or the JSON so far.
    pub fn arguments_text(&self) -> String {
        match &self.partial {
            Some(partial) => partial.clone(),
            None => pretty(&self.arguments),
        }
    }

    /// The result as the line shows it when open.
    pub fn result_text(&self) -> Option<String> {
        self.result.as_ref().map(|result| pretty(&result.output))
    }

    fn whole(&self) -> bool {
        self.partial.is_none() && !self.name.is_empty()
    }

    fn ok(&self) -> bool {
        self.whole() && self.result.as_ref().is_some_and(|result| !result.is_error)
    }
}

/// JSON as two-space indented text; a string as it is.
pub fn pretty(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// The recommendation card of a `suggest` call: from its result once it has run, else from its
/// arguments (pending).
fn recommendation(call: &ToolCall) -> Option<Recommendation> {
    let done = call.result.as_ref().filter(|result| !result.is_error).map(|result| &result.output);
    Recommendation::of(&call.arguments, done, call.result.is_none())
}

/// One thing a reply draws, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece<'a> {
    /// Collapsed `Thought for Ns`, opening to the text. `key` is unique in the reply.
    Thinking { key: String, thinking: &'a Thinking },
    /// A tool line: spinner, ✓ or ✕; collapsed, opening to the arguments and result.
    Tool { key: String, call: &'a ToolCall },
    Text(&'a str),
    /// `suggest`'s recommendation card. `key` is unique in the reply.
    Suggest { key: String, card: Recommendation },
    /// `ask_user`'s question.
    AskUser { call_id: &'a str, ask: AskUser },
}

impl Reply {
    /// What the reply draws, in order: thinking, tool lines and text as they came, then the
    /// recommendation cards, then the Advisor's question. `follow_ups` is drawn by
    /// [`Reply::follow_ups`], under these. A `suggest`, `follow_ups` or `ask_user` that failed is a
    /// tool line, showing its error.
    pub fn pieces(&self) -> Vec<Piece<'_>> {
        let (mut flow, mut cards, mut asks) = (Vec::new(), Vec::new(), Vec::new());
        for (step, parts) in self.steps.iter().enumerate() {
            for (index, part) in parts.iter().enumerate() {
                let key = format!("{step}.{index}");
                match part {
                    Part::Text(text) if !text.is_empty() => flow.push(Piece::Text(text)),
                    Part::Text(_) => {}
                    Part::Thinking(thinking) if !thinking.text.is_empty() || thinking.streaming() => flow.push(Piece::Thinking { key, thinking }),
                    Part::Thinking(_) => {}
                    Part::Tool(call) => {
                        let failed = call.status() == ToolStatus::Failed;
                        match call.name.as_str() {
                            "suggest" if call.whole() && !failed => match recommendation(call) {
                                Some(card) => cards.push(Piece::Suggest { key, card }),
                                None => flow.push(Piece::Tool { key, call }),
                            },
                            // Drawn as buttons under the reply once it has run.
                            "follow_ups" if call.whole() && !failed => {}
                            "ask_user" if call.whole() && !failed => match AskUser::parse(&call.arguments) {
                                Some(ask) => asks.push(Piece::AskUser { call_id: &call.id, ask }),
                                None => flow.push(Piece::Tool { key, call }),
                            },
                            _ => flow.push(Piece::Tool { key, call }),
                        }
                    }
                }
            }
        }
        flow.extend(cards);
        flow.extend(asks);
        flow
    }

    /// The questions of the reply's last `follow_ups` that ran: its buttons.
    pub fn follow_ups(&self) -> Vec<String> {
        let mut calls = self.steps.iter().flatten().filter_map(|part| match part {
            Part::Tool(call) if call.name == "follow_ups" && call.ok() => Some(call),
            _ => None,
        });
        let Some(call) = calls.next_back() else { return Vec::new() };
        let questions = |value: &Value| -> Option<Vec<String>> { Some(value.get("questions")?.as_array()?.iter().filter_map(Value::as_str).map(str::to_owned).collect()) };
        call.result.as_ref().and_then(|result| questions(&result.output)).or_else(|| questions(&call.arguments)).unwrap_or_default()
    }

    /// Whether something in it is visibly at work: thinking streaming, or a tool running.
    pub fn busy(&self) -> bool {
        self.steps.iter().flatten().any(|part| match part {
            Part::Thinking(thinking) => thinking.streaming(),
            Part::Tool(call) => call.status() == ToolStatus::Running,
            Part::Text(_) => false,
        })
    }

    /// It ended: thinking still streaming ends now, and a tool call with no result is interrupted.
    fn settle(&mut self, at: Instant) {
        self.open = false;
        for part in self.steps.iter_mut().flatten() {
            match part {
                Part::Thinking(thinking) if thinking.streaming() => thinking.ended = Some(at),
                Part::Tool(call) if call.result.is_none() => call.result = Some(ToolResult { is_error: true, output: Value::String(INTERRUPTED.into()) }),
                _ => {}
            }
        }
    }

    fn tool_mut(&mut self, id: &str) -> Option<&mut ToolCall> {
        self.steps.iter_mut().flatten().find_map(|part| match part {
            Part::Tool(call) if call.id == id => Some(call),
            _ => None,
        })
    }
}

impl Transcript {
    /// From the messages the API saved, in order, with the stats saved on a reply's last message.
    pub fn from_saved(messages: &[SavedMessage]) -> Self {
        let mut transcript = Self::default();
        let at = Instant::now();
        for saved in messages {
            transcript.apply(&AdvisorEvent::MessageEnd(saved.message.clone()), at);
            if let Some(situation) = saved.situation.clone().filter(|situation| !situation.is_null()) {
                transcript.set_situation(situation);
            }
            if let Some(stats) = saved.stats.as_ref().and_then(Stats::from_json)
                && let Some(reply) = transcript.last_reply_mut()
            {
                reply.stats = Some(stats);
            }
        }
        for turn in &mut transcript.turns {
            if let Turn::Reply(reply) = turn
                && reply.end == End::Streaming
            {
                reply.end = End::Done;
            }
        }
        transcript
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// The situation sent with the last question, if the last turn is a question that has none
    /// yet. Whether it was set.
    pub fn set_situation(&mut self, situation: Value) -> bool {
        match self.turns.last_mut() {
            Some(Turn::User(question)) if question.situation.is_none() => {
                question.situation = Some(situation);
                true
            }
            _ => false,
        }
    }

    /// Take in one event of a streaming reply, which came at `at`.
    pub fn apply(&mut self, event: &AdvisorEvent, at: Instant) {
        match event {
            AdvisorEvent::MessageStart(message) => match role_of(message) {
                Some("user") => {
                    self.turns.push(Turn::User(Question { text: text_of(message), situation: None }));
                    self.user_open = true;
                }
                Some("assistant") => {
                    let reply = self.streaming_reply();
                    reply.steps.push(Vec::new());
                    reply.open = true;
                }
                _ => {}
            },
            AdvisorEvent::Delta { kind, index, delta } => {
                let Some(reply) = self.last_reply_mut().filter(|reply| reply.open && reply.end == End::Streaming) else { return };
                let Some(step) = reply.steps.last_mut() else { return };
                // Anything else streaming means the thinking before it is over.
                for (other, part) in step.iter_mut().enumerate() {
                    if let Part::Thinking(thinking) = part
                        && other != *index
                        && thinking.streaming()
                    {
                        thinking.ended = Some(at);
                    }
                }
                while step.len() <= *index {
                    step.push(Part::Text(String::new()));
                }
                let part = &mut step[*index];
                match kind {
                    DeltaKind::Text => match part {
                        Part::Text(text) => text.push_str(delta),
                        other => *other = Part::Text(delta.clone()),
                    },
                    DeltaKind::Thinking => match part {
                        Part::Thinking(thinking) => thinking.text.push_str(delta),
                        other => *other = Part::Thinking(Thinking { text: delta.clone(), started: Some(at), ended: None }),
                    },
                    DeltaKind::ToolCall => match part {
                        Part::Tool(call) => call.partial.get_or_insert_with(String::new).push_str(delta),
                        other => *other = Part::Tool(ToolCall { id: String::new(), name: String::new(), arguments: Value::Null, partial: Some(delta.clone()), result: None }),
                    },
                    DeltaKind::Other(_) => {}
                }
            }
            AdvisorEvent::MessageEnd(message) => match role_of(message) {
                Some("user") => {
                    // A streamed question came with its `message_start`; a saved one comes only as this.
                    if !std::mem::take(&mut self.user_open) {
                        self.turns.push(Turn::User(Question { text: text_of(message), situation: None }));
                    }
                }
                Some("assistant") => self.end_step(message, at),
                Some("toolResult") => {
                    let id = message.get("toolCallId").and_then(Value::as_str).unwrap_or_default();
                    if let Some(call) = self.last_reply_mut().and_then(|reply| reply.tool_mut(id)) {
                        call.result = Some(result_of(message));
                    }
                }
                _ => {}
            },
            AdvisorEvent::Stats(event) => {
                if let Some(stats) = event.get("stats").and_then(Stats::from_json)
                    && let Some(reply) = self.last_reply_mut()
                {
                    reply.stats = Some(stats);
                }
            }
            AdvisorEvent::Error(error) => {
                let reply = self.streaming_reply();
                if !matches!(reply.end, End::Failed(_)) {
                    reply.end = End::Failed(error.clone());
                }
                reply.settle(at);
            }
            AdvisorEvent::Done => self.finish(at),
            AdvisorEvent::Unknown(_) => {}
        }
    }

    /// The stream ended without trouble: a reply still streaming is done.
    pub fn finish(&mut self, at: Instant) {
        if let Some(reply) = self.last_reply_mut().filter(|reply| reply.end == End::Streaming) {
            reply.end = End::Done;
            reply.settle(at);
        }
    }

    /// Stop: the reply streaming is kept as it is and marked stopped (one is shown even if nothing
    /// of it had come yet).
    pub fn stop(&mut self, at: Instant) {
        let reply = self.streaming_reply();
        reply.end = End::Stopped;
        reply.settle(at);
    }

    /// The stream broke off: the reply streaming failed with `error`.
    pub fn fail(&mut self, error: String, at: Instant) {
        if let Some(reply) = self.last_reply_mut().filter(|reply| reply.end == End::Streaming) {
            reply.end = End::Failed(error);
            reply.settle(at);
        }
    }

    /// The player's message after turn `index`: what answered an `ask_user` in it.
    pub fn answer_after(&self, index: usize) -> Option<&str> {
        self.turns.iter().skip(index + 1).find_map(|turn| match turn {
            Turn::User(question) => Some(question.text.as_str()),
            Turn::Reply(_) => None,
        })
    }

    /// The question the last reply asks and no message has answered yet: its call id and card.
    pub fn waiting_question(&self) -> Option<(String, AskUser)> {
        let Some(Turn::Reply(reply)) = self.turns.last() else { return None };
        reply.pieces().into_iter().rev().find_map(|piece| match piece {
            Piece::AskUser { call_id, ask } => Some((call_id.to_owned(), ask)),
            _ => None,
        })
    }

    /// Whether the last reply is streaming.
    pub fn streaming(&self) -> bool {
        matches!(self.turns.last(), Some(Turn::Reply(reply)) if reply.end == End::Streaming)
    }

    fn last_reply_mut(&mut self) -> Option<&mut Reply> {
        match self.turns.last_mut() {
            Some(Turn::Reply(reply)) => Some(reply),
            _ => None,
        }
    }

    /// The reply still streaming, or a new one after the last question.
    fn streaming_reply(&mut self) -> &mut Reply {
        if !matches!(self.turns.last(), Some(Turn::Reply(reply)) if reply.end == End::Streaming) {
            self.turns.push(Turn::Reply(Reply::new()));
        }
        match self.turns.last_mut() {
            Some(Turn::Reply(reply)) => reply,
            _ => unreachable!("a reply was just pushed"),
        }
    }

    /// An assistant message is whole: it replaces what streamed of it (keeping when its thinking
    /// streamed), and says how the reply stands.
    fn end_step(&mut self, message: &Value, at: Instant) {
        let reply = self.streaming_reply();
        let streamed = if reply.open { reply.steps.pop().unwrap_or_default() } else { Vec::new() };
        reply.open = false;
        let mut times = streamed.iter().filter_map(|part| match part {
            Part::Thinking(thinking) => Some((thinking.started, thinking.ended.or(thinking.started.map(|_| at)))),
            _ => None,
        });
        let mut parts = parts_of(message);
        for part in &mut parts {
            if let Part::Thinking(thinking) = part
                && let Some((started, ended)) = times.next()
            {
                thinking.started = started;
                thinking.ended = ended;
            }
        }
        if !parts.is_empty() {
            reply.steps.push(parts);
        }
        match message.get("stopReason").and_then(Value::as_str) {
            Some("aborted") => {
                reply.end = End::Stopped;
                reply.settle(at);
            }
            Some("error") => {
                let error = message.get("errorMessage").and_then(Value::as_str).filter(|error| !error.is_empty()).unwrap_or("The reply failed.");
                reply.end = End::Failed(error.to_owned());
                reply.settle(at);
            }
            _ => {
                if let Some(error) = message.get("errorMessage").and_then(Value::as_str).filter(|error| !error.is_empty()) {
                    reply.end = End::Failed(error.to_owned());
                    reply.settle(at);
                }
            }
        }
    }
}

fn role_of(message: &Value) -> Option<&str> {
    message.get("role").and_then(Value::as_str)
}

/// A message's text: its content as a string, or its text blocks joined.
fn text_of(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks.iter().filter(|block| block.get("type").and_then(Value::as_str) == Some("text")).filter_map(|block| block.get("text").and_then(Value::as_str)).collect(),
        _ => String::new(),
    }
}

/// An assistant message's content blocks as parts (a saved thinking's time isn't known).
fn parts_of(message: &Value) -> Vec<Part> {
    let Some(blocks) = message.get("content").and_then(Value::as_array) else { return Vec::new() };
    let text = |block: &Value, key: &str| block.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    blocks
        .iter()
        .filter_map(|block| match block.get("type").and_then(Value::as_str)? {
            "text" => Some(Part::Text(text(block, "text"))),
            "thinking" => Some(Part::Thinking(Thinking { text: text(block, "thinking"), started: None, ended: None })),
            "toolCall" => Some(Part::Tool(ToolCall {
                id: text(block, "id"),
                name: text(block, "name"),
                arguments: block.get("arguments").cloned().unwrap_or(Value::Object(Default::default())),
                partial: None,
                result: None,
            })),
            _ => None,
        })
        .collect()
}

/// A tool result message: its `details`, else the text the model read.
fn result_of(message: &Value) -> ToolResult {
    let is_error = message.get("isError").and_then(Value::as_bool).unwrap_or(false);
    let output = match message.get("details") {
        Some(details) if !details.is_null() => details.clone(),
        _ => {
            let text: Vec<&str> = message
                .get("content")
                .and_then(Value::as_array)
                .map(|blocks| blocks.iter().map(|block| if block.get("type").and_then(Value::as_str) == Some("text") { block.get("text").and_then(Value::as_str).unwrap_or_default() } else { "[image]" }).collect())
                .unwrap_or_default();
            // Text that is JSON is shown as JSON.
            let text = text.join("");
            serde_json::from_str::<Value>(&text).ok().filter(|value| value.is_object() || value.is_array()).unwrap_or(Value::String(text))
        }
    };
    ToolResult { is_error, output }
}

#[cfg(test)]
#[path = "reply_tests.rs"]
mod tests;
