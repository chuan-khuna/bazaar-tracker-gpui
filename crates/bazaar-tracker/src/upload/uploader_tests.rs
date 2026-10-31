use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicUsize;

use super::*;
use crate::run::{Event, EventKind, Outcome};
use crate::upload::transport;

const KEY: &str = "secret-key-123";
const NOW_MS: i64 = 10_000_000_000;

// --- a fake HTTP server ----------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Received {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Received {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// The decompressed JSON body.
    fn json(&self) -> serde_json::Value {
        let mut text = String::new();
        flate2::read::GzDecoder::new(&self.body[..]).read_to_string(&mut text).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn event_ids(&self) -> Vec<String> {
        self.json()["events"].as_array().unwrap().iter().map(|event| event["id"].as_str().unwrap().to_owned()).collect()
    }
}

struct FakeServer {
    url: String,
    received: Arc<Mutex<Vec<Received>>>,
    /// Statuses (and bodies) for the next requests; once used up, every request gets 200.
    script: Arc<Mutex<VecDeque<(u16, String)>>>,
}

impl FakeServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/api/runs/batch", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(Vec::new()));
        let script: Arc<Mutex<VecDeque<(u16, String)>>> = Arc::default();
        {
            let (received, script) = (received.clone(), script.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    handle(stream, &received, &script);
                }
            });
        }
        Self { url, received, script }
    }

    fn respond_with(&self, responses: &[(u16, &str)]) {
        self.script.lock().unwrap().extend(responses.iter().map(|(status, body)| (*status, (*body).to_owned())));
    }

    fn requests(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

fn handle(mut stream: TcpStream, received: &Mutex<Vec<Received>>, script: &Mutex<VecDeque<(u16, String)>>) {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            return;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap().split(' ');
    let (method, path) = (request_line.next().unwrap().to_owned(), request_line.next().unwrap().to_owned());
    let headers: Vec<(String, String)> = lines.filter_map(|line| line.split_once(':')).map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned())).collect();
    let length: usize = headers.iter().find(|(key, _)| key.eq_ignore_ascii_case("content-length")).and_then(|(_, value)| value.parse().ok()).unwrap_or(0);
    let mut body = buffer[header_end..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    received.lock().unwrap().push(Received { method, path, headers, body });
    let (status, text) = script.lock().unwrap().pop_front().unwrap_or((200, String::new()));
    let response = format!("HTTP/1.1 {status} Whatever\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

// --- helpers -----------------------------------------------------------------------------------

fn history() -> (tempfile::TempDir, Arc<History>) {
    let dir = tempfile::tempdir().unwrap();
    let history = Arc::new(History::open(&dir.path().join("history.db")).unwrap());
    (dir, history)
}

fn event(id: &str, run: &str, at: i64) -> Event {
    Event { id: id.to_owned(), run_id: run.to_owned(), timestamp_ms: at, day: None, hour: None, kind: EventKind::Rerolled }
}

fn ended(id: &str, run: &str, at: i64) -> Event {
    event(id, run, at).with_kind(EventKind::RunEnded { outcome: Outcome::Win, victories: Some(10), losses: Some(1), gold: None, health_max: None, prestige: None, level: None, income: None, rating_after: None, rank_after: None, rank_division_after: None })
}

trait WithKind {
    fn with_kind(self, kind: EventKind) -> Event;
}

impl WithKind for Event {
    fn with_kind(mut self, kind: EventKind) -> Event {
        self.kind = kind;
        self
    }
}

/// An ended run of `count` events, ids `<run>-<n>`.
fn ended_run(history: &History, run: &str, count: usize) {
    for n in 0..count - 1 {
        history.insert_event(&event(&format!("{run}-{n}"), run, 1000 + n as i64)).unwrap();
    }
    history.insert_event(&ended(&format!("{run}-{}", count - 1), run, 1000 + count as i64)).unwrap();
}

fn key_source(key: Option<&'static str>) -> Box<dyn KeySource> {
    Box::new(move || key.map(str::to_owned))
}

fn config(server: &FakeServer) -> UploadConfig {
    UploadConfig { endpoint: server.url.clone(), client_version: "9.9.9".into(), ..UploadConfig::default() }
}

fn uploader(history: &Arc<History>, server: &FakeServer) -> (Uploader, Receiver<UploadNotice>) {
    let (uploader, notices) = Uploader::new(history.clone(), key_source(Some(KEY)), config(server));
    uploader.set_enabled(true);
    (uploader, notices)
}

// --- tests -------------------------------------------------------------------------------------

#[test]
fn run_sync_is_off_until_turned_on() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 3);
    let (uploader, _notices) = Uploader::new(history.clone(), key_source(Some(KEY)), config(&server));
    assert!(!uploader.is_enabled(), "off by default");
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Idle);
    assert!(server.requests().is_empty());
    uploader.set_enabled(true);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 3 });
    uploader.set_enabled(false);
    ended_run(&history, "r2", 2);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Idle);
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn nothing_is_sent_without_a_key() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 3);
    for key in [None, Some(""), Some("   ")] {
        let (uploader, notices) = Uploader::new(history.clone(), key_source(key), config(&server));
        uploader.set_enabled(true);
        assert_eq!(uploader.drain_once(NOW_MS), Drain::Idle, "{key:?}");
        assert!(notices.try_recv().is_err());
    }
    assert!(server.requests().is_empty());
}

#[test]
fn a_batch_is_gzip_json_posted_with_the_headers_the_spec_names() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    history.insert_event(&event("a", "r1", 100)).unwrap();
    history.insert_event(&ended("b", "r1", 200)).unwrap();
    let (uploader, notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 2 });

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!((request.method.as_str(), request.path.as_str()), ("POST", "/api/runs/batch"));
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    assert_eq!(request.header("Content-Encoding"), Some("gzip"));
    assert_eq!(request.header("Authorization"), Some("Bearer secret-key-123"));
    assert_eq!(request.header("User-Agent"), Some("BazaarTracker/9.9.9"));

    let json = request.json();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["client_version"], "9.9.9");
    assert_eq!(json["runs"][0]["id"], "r1");
    assert_eq!(json["runs"][0]["ended"], true);
    assert_eq!(request.event_ids(), ["a", "b"]);
    assert_eq!(json["events"][0]["type"], "Rerolled");
    assert_eq!(notices.try_iter().collect::<Vec<_>>(), [UploadNotice::Sent { events: 2 }]);
}

#[test]
fn accepted_events_are_marked_sent_and_not_sent_again() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 3);
    let (uploader, _notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 3 });
    for n in 0..3 {
        assert_eq!(history.upload_status(&format!("r1-{n}")).unwrap(), Some(UploadStatus::Sent));
    }
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 0 });
    assert_eq!(server.requests().len(), 1, "nothing left to send, so no second request");
}

#[test]
fn batches_hold_at_most_200_events() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 450);
    let (uploader, _notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 450 });
    let sizes: Vec<_> = server.requests().iter().map(|request| request.event_ids().len()).collect();
    assert_eq!(sizes, [200, 200, 50]);
    // In order, with no event twice.
    let all: Vec<String> = server.requests().iter().flat_map(Received::event_ids).collect();
    assert_eq!(all.len(), 450);
    assert_eq!(all[0], "r1-0");
    assert_eq!(all[449], "r1-449");
}

#[test]
fn batches_stop_at_the_payload_byte_cap_but_always_hold_one() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 10);
    let one_event_bytes = history.waiting_events(1, i64::MAX).unwrap()[0].json.len();
    // Room for 3 events (and a bit), so 10 go as 3, 3, 3, 1.
    let mut cfg = config(&server);
    cfg.batch_bytes = one_event_bytes * 3 + one_event_bytes / 2;
    let (uploader, _notices) = Uploader::new(history.clone(), key_source(Some(KEY)), cfg);
    uploader.set_enabled(true);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 10 });
    let sizes: Vec<_> = server.requests().iter().map(|request| request.event_ids().len()).collect();
    assert_eq!(sizes, [3, 3, 3, 1]);

    // A cap smaller than any one event still sends them, one at a time.
    let (_dir2, history2) = self::history();
    ended_run(&history2, "r2", 3);
    let server2 = FakeServer::start();
    let mut tiny = config(&server2);
    tiny.batch_bytes = 1;
    let (uploader2, _n) = Uploader::new(history2.clone(), key_source(Some(KEY)), tiny);
    uploader2.set_enabled(true);
    assert_eq!(uploader2.drain_once(NOW_MS), Drain::Done { sent: 3 });
    assert_eq!(server2.requests().len(), 3);
}

#[test]
fn a_refusal_backs_off_leaves_the_events_waiting_and_reports_the_servers_message() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 3);
    server.respond_with(&[(401, r#"{"message":"That key is not valid"}"#), (503, "<html>down</html>")]);
    let (uploader, notices) = uploader(&history, &server);

    assert_eq!(uploader.drain_once(NOW_MS), Drain::Backoff { sent: 0 });
    assert_eq!(history.upload_status("r1-0").unwrap(), Some(UploadStatus::Waiting));
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Backoff { sent: 0 });
    assert_eq!(
        notices.try_iter().collect::<Vec<_>>(),
        [
            UploadNotice::Refused { status: 401, message: Some("That key is not valid".into()) },
            UploadNotice::Refused { status: 503, message: None },
        ]
    );
    // The server is back: the same events go.
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 3 });
    assert_eq!(server.requests().len(), 3);
    assert_eq!(server.requests()[2].event_ids(), ["r1-0", "r1-1", "r1-2"]);
}

#[test]
fn the_wait_after_failures_doubles_from_15_seconds_up_to_an_hour() {
    let (base, max) = (Duration::from_secs(15), Duration::from_secs(3600));
    let waits: Vec<u64> = (1..=11).map(|n| backoff_delay(n, base, max).as_secs()).collect();
    assert_eq!(waits, [15, 30, 60, 120, 240, 480, 960, 1920, 3600, 3600, 3600]);
    assert_eq!(backoff_delay(0, base, max), Duration::ZERO);
    assert_eq!(UploadConfig::default().base_delay, base);
    assert_eq!(UploadConfig::default().max_delay, max);
    assert_eq!(UploadConfig::default().idle_wake, Duration::from_secs(15));
}

#[test]
fn a_413_splits_the_batch_and_each_half_is_tried() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 4);
    // Too large as 4 and as 2 for the first half; the rest fit.
    server.respond_with(&[(413, ""), (413, "")]);
    let (uploader, notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 4 });
    let sizes: Vec<_> = server.requests().iter().map(|request| request.event_ids().len()).collect();
    assert_eq!(sizes, [4, 2, 1, 1, 2]);
    assert_eq!(notices.try_iter().filter(|notice| matches!(notice, UploadNotice::Sent { .. })).count(), 3);
    for n in 0..4 {
        assert_eq!(history.upload_status(&format!("r1-{n}")).unwrap(), Some(UploadStatus::Sent));
    }
}

#[test]
fn a_413_for_a_single_event_sets_it_aside_for_good() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "r1", 2);
    // Both halves are too large: each event is refused on its own... only the first is.
    server.respond_with(&[(413, ""), (413, ""), (200, "")]);
    let (uploader, notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 1 });
    assert_eq!(history.upload_status("r1-0").unwrap(), Some(UploadStatus::Rejected));
    assert_eq!(history.upload_status("r1-1").unwrap(), Some(UploadStatus::Sent));
    assert!(notices.try_iter().any(|notice| matches!(notice, UploadNotice::Rejected { event_id, event_type } if event_id == "r1-0" && event_type == "Rerolled")));
    // The set-aside event is never tried again.
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 0 });
    assert_eq!(server.requests().len(), 3);
}

#[test]
fn local_runs_are_not_sent_until_they_are_rebound_to_a_real_id() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "local-abc", 2);
    let (uploader, _notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 0 });
    assert!(server.requests().is_empty());
    history.rebind_run("local-abc", "real-1").unwrap();
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 2 });
    assert_eq!(server.requests()[0].json()["runs"][0]["id"], "real-1");
}

#[test]
fn a_run_still_going_waits_until_it_ends_or_goes_stale() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    let last_event_at = NOW_MS - 3_600_000; // an hour ago
    history.insert_event(&event("a", "r1", last_event_at - 10)).unwrap();
    history.insert_event(&event("b", "r1", last_event_at)).unwrap();
    let (uploader, _notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 0 }, "an hour of quiet is not stale");
    assert!(server.requests().is_empty());
    assert_eq!(uploader.drain_once(NOW_MS + 6 * 3_600_000), Drain::Done { sent: 2 }, "after six hours of quiet it is");
}

#[test]
fn only_the_first_events_of_a_long_run_wait_for_its_end() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    ended_run(&history, "done", 2);
    history.insert_event(&event("live-1", "live", NOW_MS - 1000)).unwrap();
    let (uploader, _notices) = uploader(&history, &server);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 2 });
    assert_eq!(server.requests()[0].event_ids(), ["done-0", "done-1"]);
}

#[test]
fn a_network_failure_backs_off() {
    let (_dir, history) = history();
    ended_run(&history, "r1", 2);
    let mut cfg = UploadConfig::default();
    // Nothing is listening on this port.
    let free = TcpListener::bind("127.0.0.1:0").unwrap();
    cfg.endpoint = format!("http://{}/x", free.local_addr().unwrap());
    drop(free);
    let (uploader, notices) = Uploader::new(history.clone(), key_source(Some(KEY)), cfg);
    uploader.set_enabled(true);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Backoff { sent: 0 });
    assert!(matches!(notices.try_recv().unwrap(), UploadNotice::Failed(_)));
    assert_eq!(history.upload_status("r1-0").unwrap(), Some(UploadStatus::Waiting));
}

/// A transport that answers from a script, for the cases a real server cannot easily produce.
struct Scripted {
    answers: Mutex<VecDeque<Result<transport::Response, TransportError>>>,
    sizes: Arc<Mutex<Vec<usize>>>,
    calls: AtomicUsize,
}

impl Transport for Scripted {
    fn post(&self, request: &Request<'_>) -> Result<transport::Response, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut text = String::new();
        flate2::read::GzDecoder::new(request.body).read_to_string(&mut text).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        self.sizes.lock().unwrap().push(json["events"].as_array().unwrap().len());
        self.answers.lock().unwrap().pop_front().unwrap_or(Ok(transport::Response { status: 200, body: String::new() }))
    }
}

#[test]
fn a_timeout_splits_a_batch_like_a_413_and_backs_off_for_a_single_event() {
    let (_dir, history) = history();
    ended_run(&history, "r1", 4);
    let sizes = Arc::new(Mutex::new(Vec::new()));
    let transport = Scripted { answers: Mutex::new(VecDeque::from([Err(TransportError::Timeout)])), sizes: sizes.clone(), calls: AtomicUsize::new(0) };
    let (uploader, _notices) = Uploader::with_transport(history.clone(), key_source(Some(KEY)), UploadConfig::default(), Box::new(transport));
    uploader.set_enabled(true);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Done { sent: 4 });
    assert_eq!(*sizes.lock().unwrap(), [4, 2, 2]);

    let (_dir2, history2) = self::history();
    ended_run(&history2, "r2", 1);
    let single = Scripted { answers: Mutex::new(VecDeque::from([Err(TransportError::Timeout)])), sizes: Arc::default(), calls: AtomicUsize::new(0) };
    let (uploader2, _n) = Uploader::with_transport(history2.clone(), key_source(Some(KEY)), UploadConfig::default(), Box::new(single));
    uploader2.set_enabled(true);
    assert_eq!(uploader2.drain_once(NOW_MS), Drain::Backoff { sent: 0 });
}

#[test]
fn if_the_first_half_has_to_back_off_the_second_is_not_tried() {
    let (_dir, history) = history();
    ended_run(&history, "r1", 4);
    let sizes = Arc::new(Mutex::new(Vec::new()));
    let answers = VecDeque::from([
        Ok(transport::Response { status: 413, body: String::new() }),
        Ok(transport::Response { status: 500, body: String::new() }),
    ]);
    let transport = Scripted { answers: Mutex::new(answers), sizes: sizes.clone(), calls: AtomicUsize::new(0) };
    let (uploader, _notices) = Uploader::with_transport(history.clone(), key_source(Some(KEY)), UploadConfig::default(), Box::new(transport));
    uploader.set_enabled(true);
    assert_eq!(uploader.drain_once(NOW_MS), Drain::Backoff { sent: 0 });
    assert_eq!(*sizes.lock().unwrap(), [4, 2]);
}

#[test]
fn the_endpoint_comes_from_the_environment_setting_or_the_default() {
    assert_eq!(endpoint_from(None), DEFAULT_ENDPOINT);
    assert_eq!(endpoint_from(Some("")), DEFAULT_ENDPOINT);
    assert_eq!(endpoint_from(Some("   ")), DEFAULT_ENDPOINT);
    assert_eq!(endpoint_from(Some("https://example.test/api/batch/")), "https://example.test/api/batch");
    assert_eq!(UploadConfig { endpoint: "http://x/y".into(), ..UploadConfig::default() }.endpoint, "http://x/y");
}

#[test]
fn server_error_text_is_found_in_the_usual_places_and_cut_to_200_characters() {
    assert_eq!(api_error(r#"{"message":"bad key"}"#).as_deref(), Some("bad key"));
    assert_eq!(api_error(r#"{"error":"nope"}"#).as_deref(), Some("nope"));
    assert_eq!(api_error(r#"{"detail":"d","title":"t"}"#).as_deref(), Some("d"));
    assert_eq!(api_error(r#"{"title":"t"}"#).as_deref(), Some("t"));
    assert_eq!(api_error(r#"{"error":{"message":"deep"}}"#).as_deref(), Some("deep"));
    assert_eq!(api_error("<html>"), None);
    assert_eq!(api_error(""), None);
    assert_eq!(api_error(r#"[1,2]"#), None);
    assert_eq!(api_error(r#"{"message":7}"#), None);
    let long = format!(r#"{{"message":"{}"}}"#, "x".repeat(300));
    let cut = api_error(&long).unwrap();
    assert_eq!(cut.chars().count(), 201);
    assert!(cut.ends_with('…'));
}

#[test]
fn the_background_loop_sends_backs_off_and_stops() {
    let (_dir, history) = history();
    let server = FakeServer::start();
    server.respond_with(&[(500, ""), (500, "")]);
    ended_run(&history, "r1", 2);
    let mut cfg = config(&server);
    cfg.base_delay = Duration::from_millis(20);
    cfg.max_delay = Duration::from_millis(80);
    cfg.idle_wake = Duration::from_millis(30);
    let (uploader, notices) = Uploader::new(history.clone(), key_source(Some(KEY)), cfg);
    uploader.set_enabled(true);
    let handle = Arc::new(uploader).start();

    let started = Instant::now();
    while history.upload_status("r1-0").unwrap() != Some(UploadStatus::Sent) {
        assert!(started.elapsed() < Duration::from_secs(10), "never sent");
        std::thread::sleep(Duration::from_millis(10));
    }
    // Two refusals (with waits of 20 ms then 40 ms), then the batch.
    assert_eq!(server.requests().len(), 3);
    assert!(started.elapsed() >= Duration::from_millis(60), "the loop waited between attempts");
    assert!(notices.try_iter().any(|notice| matches!(notice, UploadNotice::Sent { events: 2 })));

    // New work arrives while idle: a nudge sends it without waiting out the idle wake.
    ended_run(&history, "r2", 1);
    handle.nudge();
    let started = Instant::now();
    while history.upload_status("r2-0").unwrap() != Some(UploadStatus::Sent) {
        assert!(started.elapsed() < Duration::from_secs(10), "never sent");
        std::thread::sleep(Duration::from_millis(5));
    }
    let stopping = Instant::now();
    handle.stop();
    assert!(stopping.elapsed() < Duration::from_secs(3));
}
