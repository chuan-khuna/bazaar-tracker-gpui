//! Uploading saved Events to Bazaar DB in the background, under our own name, with back-off on
//! failure and without ever blocking tracking.
//!
//! - Nothing is sent while run sync is off (the default) or no secret key is saved.
//! - An Event is sendable once, and only once: it has not been uploaded, its Run has a real id (not
//!   `local-`), and its Run has ended or has had no new Event for six hours.
//! - Up to 200 Events and about 2,000,000 bytes of Event JSON go in one batch (always at least one),
//!   gzip-compressed, with `Authorization: Bearer <key>` and `User-Agent: BazaarTracker/<version>`.
//! - A refusal or a failure backs off (15 s, doubling, up to an hour) and leaves the Events waiting.
//!   A `413` splits the batch in two and tries each half; a `413` for a single Event sets it aside for good.
//!
//! What the real server accepts is not known (feasibility record, open question 3), so the body
//! format here is our own and behaviour against the real server is unverified.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::batch::{self, MAX_BATCH_EVENTS, MAX_BATCH_PAYLOAD_BYTES};
use super::secret::SecretStore;
use super::transport::{Request, Transport, TransportError, UreqTransport};
use crate::history::{History, UploadStatus, WaitingEvent};
use crate::run::Run;

/// Where batches go unless told otherwise.
pub const DEFAULT_ENDPOINT: &str = "https://adira.bzdb.network/api/tracker-v2/runs/batch";
/// The environment variable that overrides the endpoint.
pub const ENDPOINT_ENV: &str = "BAZAAR_TRACKER_UPLOAD_ENDPOINT";
/// How much of a server's error text is kept.
const ERROR_TEXT_LIMIT: usize = 200;

/// The endpoint: [`ENDPOINT_ENV`] if it is set to something, otherwise [`DEFAULT_ENDPOINT`], without
/// a trailing slash.
pub fn default_endpoint() -> String {
    endpoint_from(std::env::var(ENDPOINT_ENV).ok().as_deref())
}

fn endpoint_from(setting: Option<&str>) -> String {
    setting.map(str::trim).filter(|value| !value.is_empty()).unwrap_or(DEFAULT_ENDPOINT).trim_end_matches('/').to_owned()
}

#[derive(Clone, Debug)]
pub struct UploadConfig {
    pub endpoint: String,
    /// Sent as `BazaarTracker/<version>` and in the body.
    pub client_version: String,
    pub batch_events: usize,
    pub batch_bytes: usize,
    /// How long a Run may go without a new Event before its Events can be uploaded without the Run ending.
    pub stale_after: Duration,
    /// How long to wait between looks when all is well.
    pub idle_wake: Duration,
    /// The first back-off, which doubles with each failure in a row up to `max_delay`.
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            endpoint: default_endpoint(),
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
            batch_events: MAX_BATCH_EVENTS,
            batch_bytes: MAX_BATCH_PAYLOAD_BYTES,
            stale_after: Duration::from_secs(6 * 3600),
            idle_wake: Duration::from_secs(15),
            base_delay: Duration::from_secs(15),
            max_delay: Duration::from_secs(3600),
        }
    }
}

/// How long to wait after `failures` failures in a row: `base` doubling each time, up to `max`.
pub fn backoff_delay(failures: u32, base: Duration, max: Duration) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    let doublings = (failures - 1).min(8);
    (base * 2u32.pow(doublings)).min(max)
}

/// Where the secret key comes from.
pub trait KeySource: Send + Sync {
    fn key(&self) -> Option<String>;
}

impl KeySource for SecretStore {
    fn key(&self) -> Option<String> {
        self.load().ok().flatten()
    }
}

impl<F: Fn() -> Option<String> + Send + Sync> KeySource for F {
    fn key(&self) -> Option<String> {
        self()
    }
}

/// What the uploader reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UploadNotice {
    /// The server accepted this many Events.
    Sent { events: usize },
    /// The server refused a batch (any status but success and 413); the Events stay waiting.
    Refused { status: u16, message: Option<String> },
    /// The server said this Event is too large; it is set aside for good.
    Rejected { event_id: String, event_type: String },
    /// A batch could not be sent or recorded; the Events stay waiting.
    Failed(String),
}

/// What one pass did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drain {
    /// Sync is off or there is no key: nothing was looked at.
    Idle,
    /// Everything that could be sent was; this many Events were accepted.
    Done { sent: usize },
    /// The server or the network said to back off.
    Backoff { sent: usize },
}

impl Drain {
    pub fn is_backoff(&self) -> bool {
        matches!(self, Self::Backoff { .. })
    }
}

pub struct Uploader {
    history: Arc<History>,
    transport: Box<dyn Transport>,
    keys: Box<dyn KeySource>,
    config: UploadConfig,
    enabled: AtomicBool,
    notices: Sender<UploadNotice>,
}

impl Uploader {
    /// An uploader over the real network. Run sync starts off.
    pub fn new(history: Arc<History>, keys: Box<dyn KeySource>, config: UploadConfig) -> (Self, Receiver<UploadNotice>) {
        Self::with_transport(history, keys, config, Box::new(UreqTransport::default()))
    }

    pub fn with_transport(history: Arc<History>, keys: Box<dyn KeySource>, config: UploadConfig, transport: Box<dyn Transport>) -> (Self, Receiver<UploadNotice>) {
        let (notices, receiver) = mpsc::channel();
        (Self { history, transport, keys, config, enabled: AtomicBool::new(false), notices }, receiver)
    }

    /// Turn run sync on or off.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn notify(&self, notice: UploadNotice) {
        let _ = self.notices.send(notice);
    }

    /// One pass: send what is waiting, batch after batch, until nothing is, or the server says to
    /// back off. `now_ms` decides which Runs have gone stale.
    pub fn drain_once(&self, now_ms: i64) -> Drain {
        if !self.is_enabled() {
            return Drain::Idle;
        }
        let Some(key) = self.keys.key().filter(|key| !key.trim().is_empty()) else { return Drain::Idle };
        let stale_before = now_ms - self.config.stale_after.as_millis() as i64;
        let mut sent = 0;
        loop {
            let waiting = match self.history.waiting_events(self.config.batch_events, stale_before) {
                Ok(waiting) => waiting,
                Err(error) => {
                    self.notify(UploadNotice::Failed(format!("could not read the history: {error}")));
                    return Drain::Backoff { sent };
                }
            };
            if waiting.is_empty() {
                return Drain::Done { sent };
            }
            let batch = batch::limit_by_size(waiting, self.config.batch_bytes);
            match self.send_batch(&batch, &key) {
                Ok(accepted) => sent += accepted,
                Err(Backoff) => return Drain::Backoff { sent },
            }
        }
    }

    /// Send one batch. `Ok` is how many Events the server accepted (a set-aside Event is not one).
    fn send_batch(&self, batch: &[WaitingEvent], key: &str) -> Result<usize, Backoff> {
        if batch.is_empty() {
            return Ok(0);
        }
        let mut runs: Vec<Run> = Vec::new();
        for event in batch {
            if !runs.iter().any(|run| run.id == event.event.run_id)
                && let Ok(Some(run)) = self.history.get_run(&event.event.run_id) {
                    runs.push(run);
                }
        }
        let body = batch::gzip(&batch::build_body(&self.config.client_version, &runs, batch));
        let user_agent = format!("BazaarTracker/{}", self.config.client_version);
        let request = Request { url: &self.config.endpoint, api_key: key, user_agent: &user_agent, body: &body };

        match self.transport.post(&request) {
            Ok(response) if (200..300).contains(&response.status) => {
                let ids: Vec<String> = batch.iter().map(|event| event.event.id.clone()).collect();
                if let Err(error) = self.history.set_upload_status(&ids, UploadStatus::Sent) {
                    self.notify(UploadNotice::Failed(format!("the batch was accepted but could not be recorded: {error}")));
                    return Err(Backoff);
                }
                self.notify(UploadNotice::Sent { events: batch.len() });
                Ok(batch.len())
            }
            Ok(response) if response.status == 413 && batch.len() > 1 => self.split_and_retry(batch, key),
            Ok(response) if response.status == 413 => {
                let event = &batch[0].event;
                if let Err(error) = self.history.set_upload_status(std::slice::from_ref(&event.id), UploadStatus::Rejected) {
                    self.notify(UploadNotice::Failed(format!("an event was refused for good but could not be set aside: {error}")));
                    return Err(Backoff);
                }
                self.notify(UploadNotice::Rejected { event_id: event.id.clone(), event_type: event.kind.name().to_owned() });
                Ok(0)
            }
            Ok(response) => {
                self.notify(UploadNotice::Refused { status: response.status, message: api_error(&response.body) });
                Err(Backoff)
            }
            Err(TransportError::Timeout) if batch.len() > 1 => self.split_and_retry(batch, key),
            Err(TransportError::Timeout) => {
                self.notify(UploadNotice::Failed("the upload timed out".to_owned()));
                Err(Backoff)
            }
            Err(TransportError::Other(message)) => {
                self.notify(UploadNotice::Failed(message));
                Err(Backoff)
            }
        }
    }

    /// Send the two halves of a batch the server found too large (or too slow). If the first half
    /// has to back off, the second is not tried.
    fn split_and_retry(&self, batch: &[WaitingEvent], key: &str) -> Result<usize, Backoff> {
        let (first, second) = batch.split_at(batch.len() / 2);
        Ok(self.send_batch(first, key)? + self.send_batch(second, key)?)
    }

    /// Run the uploader on its own thread until the handle is stopped.
    pub fn start(self: Arc<Self>) -> UploadHandle {
        let (signals, inbox) = mpsc::channel();
        let uploader = self;
        let thread = std::thread::Builder::new().name("uploader".into()).spawn(move || uploader.run(inbox)).expect("the operating system can start a thread");
        UploadHandle { signals, thread: Mutex::new(Some(thread)) }
    }

    fn run(&self, inbox: Receiver<Signal>) {
        let mut failures = 0u32;
        loop {
            let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64);
            failures = if self.drain_once(now_ms).is_backoff() { failures + 1 } else { 0 };
            let delay = if failures == 0 { self.config.idle_wake } else { backoff_delay(failures, self.config.base_delay, self.config.max_delay) };
            let deadline = Instant::now() + delay;
            loop {
                match inbox.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    // New work wakes an idle uploader; a backing-off one keeps waiting.
                    Ok(Signal::Nudge) if failures == 0 => break,
                    Ok(Signal::Nudge) => {}
                    // A new key or turning sync on is a fresh start.
                    Ok(Signal::Fresh) => {
                        failures = 0;
                        break;
                    }
                    Ok(Signal::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => break,
                }
            }
        }
    }
}

/// The marker for "the server or network said to back off".
#[derive(Debug)]
struct Backoff;

enum Signal {
    Nudge,
    Fresh,
    Stop,
}

/// A running uploader thread.
pub struct UploadHandle {
    signals: Sender<Signal>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl UploadHandle {
    /// There may be new Events to send. An uploader that is backing off does not hurry.
    pub fn nudge(&self) {
        let _ = self.signals.send(Signal::Nudge);
    }

    /// The key changed or sync was turned on: forget past failures and try now.
    pub fn restart_backoff(&self) {
        let _ = self.signals.send(Signal::Fresh);
    }

    pub fn stop(self) {
        self.shut_down();
    }

    fn shut_down(&self) {
        let _ = self.signals.send(Signal::Stop);
        if let Some(thread) = self.thread.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take() {
            let _ = thread.join();
        }
    }
}

impl Drop for UploadHandle {
    fn drop(&mut self) {
        self.shut_down();
    }
}

/// The message in a server's error body: the first of `message`, `error`, `detail` and `title` that
/// is text, or `error.message`; cut to 200 characters.
pub fn api_error(body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let object = json.as_object()?;
    let text = ["message", "error", "detail", "title"]
        .iter()
        .find_map(|key| object.get(*key).and_then(|value| value.as_str()))
        .or_else(|| object.get("error").and_then(|error| error.get("message")).and_then(|message| message.as_str()))?;
    if text.chars().count() > ERROR_TEXT_LIMIT {
        return Some(text.chars().take(ERROR_TEXT_LIMIT).chain(std::iter::once('…')).collect());
    }
    Some(text.to_owned())
}

#[cfg(test)]
#[path = "uploader_tests.rs"]
mod tests;
