//! The HTTP client for `just advisor serve`, and the threads that run it.
//!
//! Each [`Request`] runs on a short-lived `advisor-client` thread with a blocking `ureq` call, so
//! a reply streaming for a minute never holds up listing or deleting. What comes back, the reply's
//! events as their SSE lines are parsed, goes to the window as an [`Update`] over an
//! `async_channel`, as the tracker's notices do.

use std::io::BufRead;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use super::conversation::Conversation;
use super::sse::{AdvisorEvent, SseParser};
use super::state::Request;

/// Where `just advisor serve` listens: localhost only, port 8420.
pub const BASE_URL: &str = "http://127.0.0.1:8420";

/// How long to wait for the backend to take the connection, so a backend that isn't running is
/// found out at once.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// How long a call that is not a reply may take.
const CALL_TIMEOUT: Duration = Duration::from_secs(15);

/// Why a call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// Nothing answered: the backend isn't running.
    Unreachable,
    /// The backend answered with an error status and its `detail`. 503 means it is running but
    /// can't use `history.db` (the detail says why).
    Status { status: u16, detail: String },
    /// Anything else: a reply that isn't what the API sends, or the connection dropped.
    Other(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable => write!(f, "the Advisor isn't running"),
            Self::Status { status, detail } if detail.is_empty() => write!(f, "the Advisor answered {status}"),
            Self::Status { detail, .. } => write!(f, "{detail}"),
            Self::Other(why) => write!(f, "{why}"),
        }
    }
}

/// A Conversation's saved messages: `{running, messages: [{seq, message, stats, createdMs}]}`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Saved {
    /// Whether a reply is streaming now (from another stream than this app's).
    #[serde(default)]
    pub running: bool,
    pub messages: Vec<SavedMessage>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SavedMessage {
    pub seq: i64,
    pub message: Value,
}

/// The API's calls.
pub struct Client {
    base: String,
    /// For calls that answer at once.
    agent: ureq::Agent,
    /// For replies, which stream for as long as the model takes.
    streaming: ureq::Agent,
}

impl Client {
    /// A client for the backend at `base`, such as [`BASE_URL`].
    pub fn new(base: impl Into<String>) -> Self {
        let agent = |global: Option<Duration>| -> ureq::Agent {
            ureq::Agent::config_builder()
                .http_status_as_error(false)
                // The backend is on this machine: never through a proxy.
                .proxy(None)
                .timeout_connect(Some(CONNECT_TIMEOUT))
                .timeout_global(global)
                .build()
                .into()
        };
        Self { base: base.into().trim_end_matches('/').to_owned(), agent: agent(Some(CALL_TIMEOUT)), streaming: agent(None) }
    }

    /// `GET /api/runs/{run_id}/conversations`: newest first.
    pub fn conversations(&self, run_id: &str) -> Result<Vec<Conversation>, ClientError> {
        let response = self.agent.get(format!("{}/api/runs/{}/conversations", self.base, encode(run_id))).call();
        json_of(response)
    }

    /// `POST /api/runs/{run_id}/conversations` with the first question (its title) and the situation
    /// (its Day and Hour).
    pub fn create(&self, run_id: &str, text: &str, situation: &Value) -> Result<Conversation, ClientError> {
        let body = json!({"text": text, "situation": situation}).to_string();
        let response = self.agent.post(format!("{}/api/runs/{}/conversations", self.base, encode(run_id))).header("Content-Type", "application/json").send(body);
        json_of(response)
    }

    /// `DELETE /api/conversations/{id}`.
    pub fn delete(&self, id: &str) -> Result<(), ClientError> {
        let response = self.agent.delete(format!("{}/api/conversations/{}", self.base, encode(id))).call();
        checked(response).map(|_| ())
    }

    /// `GET /api/conversations/{id}/messages`.
    pub fn messages(&self, id: &str) -> Result<Saved, ClientError> {
        let response = self.agent.get(format!("{}/api/conversations/{}/messages", self.base, encode(id))).call();
        json_of(response)
    }

    /// `POST /api/conversations/{id}/messages`: send the question with the situation, and hand each
    /// event of the reply to `on_event` as its frame ends, until the stream ends.
    pub fn send(&self, id: &str, text: &str, situation: &Value, mut on_event: impl FnMut(AdvisorEvent)) -> Result<(), ClientError> {
        let body = json!({"text": text, "situation": situation}).to_string();
        let response = self
            .streaming
            .post(format!("{}/api/conversations/{}/messages", self.base, encode(id)))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .send(body);
        let response = checked(response)?;
        let mut reader = std::io::BufReader::new(response.into_body().into_reader());
        let mut parser = SseParser::default();
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    for frame in parser.feed(&String::from_utf8_lossy(&line)) {
                        on_event(AdvisorEvent::from_frame(&frame));
                    }
                }
                Err(error) => return Err(ClientError::Other(format!("the reply stopped: {error}"))),
            }
        }
        if let Some(frame) = parser.finish() {
            on_event(AdvisorEvent::from_frame(&frame));
        }
        Ok(())
    }
}

/// What a request gave, for the window.
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    Conversations { run_id: String, result: Result<Vec<Conversation>, ClientError> },
    /// A new Conversation, made for the first question: `text` is then sent to it with `situation`.
    Created { run_id: String, text: String, situation: Value, result: Result<Conversation, ClientError> },
    Messages { conversation_id: String, result: Result<Saved, ClientError> },
    /// One event of a reply streaming.
    Event { conversation_id: String, event: AdvisorEvent },
    /// The reply's stream ended: whole, or why not.
    Ended { conversation_id: String, result: Result<(), ClientError> },
    Deleted { conversation_id: String, result: Result<(), ClientError> },
}

/// Runs requests off the window's thread and reports what they give.
pub struct Advisor {
    client: Arc<Client>,
    updates: async_channel::Sender<Update>,
}

impl Advisor {
    /// A runner for the backend at `base`, and where its updates arrive.
    pub fn new(base: impl Into<String>) -> (Self, async_channel::Receiver<Update>) {
        let (updates, receiver) = async_channel::unbounded();
        (Self { client: Arc::new(Client::new(base)), updates }, receiver)
    }

    /// Run `request` on a thread of its own.
    pub fn ask(&self, request: Request) {
        let client = self.client.clone();
        let updates = self.updates.clone();
        let spawned = std::thread::Builder::new().name("advisor-client".into()).spawn(move || run(&client, request, &updates));
        if let Err(error) = spawned {
            tracing::error!("advisor: could not start a thread: {error}");
        }
    }
}

/// Run one request, sending what it gives to `updates`. A closed channel (the window has gone) ends it.
pub fn run(client: &Client, request: Request, updates: &async_channel::Sender<Update>) {
    let update = match request {
        Request::List { run_id } => {
            let result = client.conversations(&run_id);
            Update::Conversations { run_id, result }
        }
        Request::Create { run_id, text, situation } => {
            let result = client.create(&run_id, &text, &situation);
            Update::Created { run_id, text, situation, result }
        }
        Request::Messages { conversation_id } => {
            let result = client.messages(&conversation_id);
            Update::Messages { conversation_id, result }
        }
        Request::Delete { conversation_id } => {
            let result = client.delete(&conversation_id);
            Update::Deleted { conversation_id, result }
        }
        Request::Send { conversation_id, text, situation } => {
            let result = client.send(&conversation_id, &text, &situation, |event| {
                let _ = updates.send_blocking(Update::Event { conversation_id: conversation_id.clone(), event });
            });
            if let Err(error) = &result {
                tracing::warn!("advisor: the reply failed: {error}");
            }
            Update::Ended { conversation_id, result }
        }
    };
    let _ = updates.send_blocking(update);
}

/// The response if its status is a success; otherwise the error, with the API's `detail`.
fn checked(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<ureq::http::Response<ureq::Body>, ClientError> {
    let mut response = match response {
        Ok(response) => response,
        Err(ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound | ureq::Error::Timeout(_)) => return Err(ClientError::Unreachable),
        Err(error) => return Err(ClientError::Other(error.to_string())),
    };
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(response);
    }
    let body = response.body_mut().read_to_string().unwrap_or_default();
    Err(ClientError::Status { status, detail: detail_of(&body) })
}

/// The API's `{"detail": "…"}`, or the body as it is.
fn detail_of(body: &str) -> String {
    match serde_json::from_str::<Value>(body).ok().as_ref().and_then(|json| json.get("detail")) {
        Some(Value::String(detail)) => detail.clone(),
        Some(other) => other.to_string(),
        None => body.trim().to_owned(),
    }
}

fn json_of<T: serde::de::DeserializeOwned>(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<T, ClientError> {
    let mut response = checked(response)?;
    let body = response.body_mut().read_to_string().map_err(|error| ClientError::Other(format!("could not read the Advisor's answer: {error}")))?;
    serde_json::from_str(&body).map_err(|error| ClientError::Other(format!("the Advisor's answer was not understood: {error}")))
}

/// A path segment with everything but the unreserved characters percent-encoded.
pub fn encode(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
