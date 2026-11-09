//! The HTTP client for `just advisor serve`, and the threads that run it.
//!
//! Each [`Request`] runs on a short-lived `advisor-client` thread with a blocking `ureq` call, so
//! a reply streaming for a minute never holds up listing or deleting. What comes back, the reply's
//! events as their SSE lines are parsed, goes to the window as an [`Update`] over an
//! `async_channel`, as the tracker's notices do. A reply streams over a plain socket that Stop
//! can shut from the window's thread.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, PoisonError};
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
    /// The reply's usage, on its last assistant message only.
    #[serde(default)]
    pub stats: Option<Value>,
    /// The situation sent with it, on a question sent with one.
    #[serde(default)]
    pub situation: Option<Value>,
}

/// One model the picker offers: `{id, name, inputUsdPerMillion?, outputUsdPerMillion?}` from
/// `GET /api/models`. The prices are USD per million tokens, missing when the endpoint gives none.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub input_usd_per_million: Option<f64>,
    #[serde(default)]
    pub output_usd_per_million: Option<f64>,
}

impl ModelOption {
    /// Its prices for the picker, such as `$3 in · $15 out /M`, `free`, or `None` when unknown.
    pub fn price_label(&self) -> Option<String> {
        let (input, output) = (self.input_usd_per_million?, self.output_usd_per_million?);
        if input == 0.0 && output == 0.0 {
            return Some("free".to_owned());
        }
        Some(format!("${} in · ${} out /M", usd(input), usd(output)))
    }
}

/// A price as short as it can be written: `3`, `0.15`, `0.075`.
fn usd(price: f64) -> String {
    let text = format!("{price:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `GET /api/models`: the Advisor's default model (its `ADVISOR_MODEL`) and the models to choose from.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ModelList {
    pub default: String,
    #[serde(default)]
    pub models: Vec<ModelOption>,
}

/// The API's calls.
pub struct Client {
    base: String,
    /// For calls that answer at once. A reply streams over a socket of its own (see [`Client::send`]).
    agent: ureq::Agent,
}

impl Client {
    /// A client for the backend at `base`, such as [`BASE_URL`].
    pub fn new(base: impl Into<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            // The backend is on this machine: never through a proxy.
            .proxy(None)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(CALL_TIMEOUT))
            .build()
            .into();
        Self { base: base.into().trim_end_matches('/').to_owned(), agent }
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

    /// `GET /api/models`: the default model and the list the model picker offers.
    pub fn models(&self) -> Result<ModelList, ClientError> {
        let response = self.agent.get(format!("{}/api/models", self.base)).call();
        json_of(response)
    }

    /// `GET /api/conversations/{id}/messages`.
    pub fn messages(&self, id: &str) -> Result<Saved, ClientError> {
        let response = self.agent.get(format!("{}/api/conversations/{}/messages", self.base, encode(id))).call();
        json_of(response)
    }

    /// `POST /api/conversations/{id}/messages`: send the question with the situation (and the model
    /// to answer with, if one was chosen), and hand each event of the reply to `on_event` as its
    /// frame ends, until the stream ends or `stop` closes it.
    ///
    /// The reply is read from a socket of its own rather than through `ureq`, so that Stop can shut
    /// it from the window's thread at once: the backend sees the stream close and cancels the reply.
    /// A stream closed by `stop` ends without an error.
    pub fn send(&self, id: &str, text: &str, situation: &Value, model: Option<&str>, stop: &StopHandle, mut on_event: impl FnMut(AdvisorEvent)) -> Result<(), ClientError> {
        let body = message_body(text, situation, model).to_string();
        let host = self.base.strip_prefix("http://").unwrap_or(&self.base).to_owned();
        let address = host.to_socket_addrs().ok().and_then(|mut addresses| addresses.next()).ok_or(ClientError::Unreachable)?;
        let socket = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).map_err(|_| ClientError::Unreachable)?;
        if !stop.attach(&socket) {
            return Ok(());
        }
        // An error after Stop is the socket being shut: not a failure.
        let broke = |error: std::io::Error| if stop.stopped() { Ok(()) } else { Err(ClientError::Other(format!("the reply stopped: {error}"))) };
        let request = format!(
            "POST /api/conversations/{}/messages HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            encode(id),
            body.len()
        );
        if let Err(error) = (&socket).write_all(request.as_bytes()) {
            return broke(error);
        }
        let mut reader = BufReader::new(&socket);
        let head = match read_head(&mut reader) {
            Ok(head) => head,
            Err(error) => return broke(error),
        };
        let mut body = BufReader::new(head.body(reader));
        if !(200..300).contains(&head.status) {
            let mut text = String::new();
            let _ = body.read_to_string(&mut text);
            return Err(ClientError::Status { status: head.status, detail: detail_of(&text) });
        }
        let mut parser = SseParser::default();
        let mut line = Vec::new();
        loop {
            line.clear();
            match body.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    for frame in parser.feed(&String::from_utf8_lossy(&line)) {
                        on_event(AdvisorEvent::from_frame(&frame));
                    }
                }
                Err(error) => return broke(error),
            }
            if stop.stopped() {
                return Ok(());
            }
        }
        if let Some(frame) = parser.finish() {
            on_event(AdvisorEvent::from_frame(&frame));
        }
        Ok(())
    }
}

/// A message's body: `{text, situation}`, with `model` only when one was chosen (else the backend
/// answers with its `ADVISOR_MODEL`).
pub fn message_body(text: &str, situation: &Value, model: Option<&str>) -> Value {
    let mut body = json!({"text": text, "situation": situation});
    if let Some(model) = model {
        body["model"] = json!(model);
    }
    body
}

/// Shuts a reply's stream from another thread: Stop.
#[derive(Clone, Debug, Default)]
pub struct StopHandle(Arc<Mutex<StopState>>);

#[derive(Debug, Default)]
struct StopState {
    stopped: bool,
    /// The reply's socket, once it is open.
    socket: Option<TcpStream>,
}

impl StopHandle {
    /// Close the stream, now or as soon as it opens.
    pub fn stop(&self) {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state.stopped = true;
        if let Some(socket) = state.socket.take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    pub fn stopped(&self) -> bool {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).stopped
    }

    /// Keep a handle on the stream's socket; false (and the socket shut) when already stopped.
    fn attach(&self, socket: &TcpStream) -> bool {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if state.stopped {
            let _ = socket.shutdown(Shutdown::Both);
            return false;
        }
        state.socket = socket.try_clone().ok();
        true
    }
}

/// A response's status, and how its body ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub status: u16,
    pub chunked: bool,
    pub length: Option<u64>,
}

impl Head {
    /// The body that follows the head in `reader`: de-chunked, cut to its length, or up to the end.
    fn body<'a, R: BufRead + 'a>(&self, reader: R) -> Box<dyn Read + 'a> {
        if self.chunked {
            Box::new(Chunked::new(reader))
        } else if let Some(length) = self.length {
            Box::new(reader.take(length))
        } else {
            Box::new(reader)
        }
    }
}

/// An HTTP/1.1 response's head: the status line and the headers, up to the blank line.
pub fn read_head(reader: &mut impl BufRead) -> std::io::Result<Head> {
    let bad = |why: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, why.to_owned());
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Err(bad("the connection closed before an answer"));
    }
    let status = line.split_whitespace().nth(1).and_then(|status| status.parse().ok()).ok_or_else(|| bad("not an HTTP answer"))?;
    let mut head = Head { status, chunked: false, length: None };
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("transfer-encoding") && value.to_ascii_lowercase().contains("chunked") {
                head.chunked = true;
            } else if name.eq_ignore_ascii_case("content-length") {
                head.length = value.parse().ok();
            }
        }
    }
    Ok(head)
}

/// A chunked body (as uvicorn sends a stream) read as plain bytes.
pub struct Chunked<R> {
    reader: R,
    /// What is left of the chunk being read.
    left: u64,
    done: bool,
}

impl<R: BufRead> Chunked<R> {
    pub fn new(reader: R) -> Self {
        Self { reader, left: 0, done: false }
    }
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.done || buffer.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                self.done = true;
                return Ok(0);
            }
            let size = line.trim().split(';').next().unwrap_or_default().trim().to_owned();
            self.left = u64::from_str_radix(&size, 16).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("a bad chunk size: {size:?}")))?;
            if self.left == 0 {
                // The last chunk, then trailers (none expected) up to a blank line.
                loop {
                    line.clear();
                    if self.reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
                        break;
                    }
                }
                self.done = true;
                return Ok(0);
            }
        }
        let want = buffer.len().min(usize::try_from(self.left).unwrap_or(usize::MAX));
        let read = self.reader.read(&mut buffer[..want])?;
        if read == 0 {
            self.done = true;
            return Ok(0);
        }
        self.left -= read as u64;
        if self.left == 0 {
            // The chunk's own line ending.
            let mut end = String::new();
            self.reader.read_line(&mut end)?;
        }
        Ok(read)
    }
}

/// What a request gave, for the window.
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    Conversations { run_id: String, result: Result<Vec<Conversation>, ClientError> },
    /// A new Conversation, made for the first question: `text` is then sent to it with `situation`
    /// and `model`.
    Created { run_id: String, text: String, situation: Value, model: Option<String>, result: Result<Conversation, ClientError> },
    Models { result: Result<ModelList, ClientError> },
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
    /// The replies streaming, by Conversation, so Stop can close one.
    streams: Arc<Mutex<HashMap<String, StopHandle>>>,
}

impl Advisor {
    /// A runner for the backend at `base`, and where its updates arrive.
    pub fn new(base: impl Into<String>) -> (Self, async_channel::Receiver<Update>) {
        let (updates, receiver) = async_channel::unbounded();
        (Self { client: Arc::new(Client::new(base)), updates, streams: Arc::default() }, receiver)
    }

    /// Run `request` on a thread of its own. Stop is done at once, on this thread: it closes the
    /// Conversation's stream, whose thread then ends.
    pub fn ask(&self, request: Request) {
        if let Request::Stop { conversation_id } = &request {
            if let Some(stop) = self.streams.lock().unwrap_or_else(PoisonError::into_inner).get(conversation_id) {
                stop.stop();
            }
            return;
        }
        let stop = StopHandle::default();
        if let Request::Send { conversation_id, .. } = &request {
            self.streams.lock().unwrap_or_else(PoisonError::into_inner).insert(conversation_id.clone(), stop.clone());
        }
        let client = self.client.clone();
        let updates = self.updates.clone();
        let streams = self.streams.clone();
        let spawned = std::thread::Builder::new().name("advisor-client".into()).spawn(move || {
            let conversation_id = match &request {
                Request::Send { conversation_id, .. } => Some(conversation_id.clone()),
                _ => None,
            };
            run(&client, request, &stop, &updates);
            if let Some(conversation_id) = conversation_id {
                let mut streams = streams.lock().unwrap_or_else(PoisonError::into_inner);
                // Only its own: a newer reply may have taken the place.
                if streams.get(&conversation_id).is_some_and(|known| Arc::ptr_eq(&known.0, &stop.0)) {
                    streams.remove(&conversation_id);
                }
            }
        });
        if let Err(error) = spawned {
            tracing::error!("advisor: could not start a thread: {error}");
        }
    }
}

/// Run one request, sending what it gives to `updates`; `stop` closes a reply's stream. A closed
/// channel (the window has gone) ends it.
pub fn run(client: &Client, request: Request, stop: &StopHandle, updates: &async_channel::Sender<Update>) {
    let update = match request {
        Request::List { run_id } => {
            let result = client.conversations(&run_id);
            Update::Conversations { run_id, result }
        }
        Request::Create { run_id, text, situation, model } => {
            let result = client.create(&run_id, &text, &situation);
            Update::Created { run_id, text, situation, model, result }
        }
        Request::Models => Update::Models { result: client.models() },
        Request::Messages { conversation_id } => {
            let result = client.messages(&conversation_id);
            Update::Messages { conversation_id, result }
        }
        Request::Delete { conversation_id } => {
            let result = client.delete(&conversation_id);
            Update::Deleted { conversation_id, result }
        }
        // Done on the window's thread by `Advisor::ask`.
        Request::Stop { .. } => return,
        Request::Send { conversation_id, text, situation, model } => {
            let result = client.send(&conversation_id, &text, &situation, model.as_deref(), stop, |event| {
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
