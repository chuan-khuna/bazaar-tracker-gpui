use std::io::{Read, Write};
use std::net::TcpListener;

use super::*;
use crate::tracking::advisor::sse::DeltaKind;

/// A server on a free local port that answers one request with `response`, and hands back what
/// it was asked.
/// An answer with a JSON body.
fn json_answer(status: &str, body: &str) -> String {
    format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn serve_once(response: impl Into<String>) -> (String, std::thread::JoinHandle<String>) {
    let response: String = response.into();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut asked = Vec::new();
        let mut buffer = [0u8; 4096];
        // Read the head, then as much body as it says.
        loop {
            let read = stream.read(&mut buffer).unwrap();
            asked.extend_from_slice(&buffer[..read]);
            let text = String::from_utf8_lossy(&asked).to_string();
            if let Some(end) = text.find("\r\n\r\n") {
                let length = text.lines().find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|value| value.trim().parse::<usize>().unwrap())).unwrap_or(0);
                if asked.len() >= end + 4 + length {
                    break;
                }
            }
            if read == 0 {
                break;
            }
        }
        stream.write_all(response.as_bytes()).unwrap();
        String::from_utf8_lossy(&asked).to_string()
    });
    (base, thread)
}

#[test]
fn conversations_are_listed() {
    let (base, server) = serve_once(json_answer("200 OK", r#"[{"id": "c1", "runId": "r 1", "title": "Hi", "day": 5, "hour": 2, "createdMs": 9}]"#));
    let listed = Client::new(base).conversations("r 1").unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].label(), "Hi D5 H2");
    assert!(server.join().unwrap().starts_with("GET /api/runs/r%201/conversations "));
}

#[test]
fn an_error_status_carries_its_detail() {
    let (base, server) = serve_once(json_answer("503 Service Unavailable", r#"{"detail": "history.db is at 8."}"#));
    let error = Client::new(base).messages("c1").unwrap_err();
    assert_eq!(error, ClientError::Status { status: 503, detail: "history.db is at 8.".into() });
    server.join().unwrap();
}

#[test]
fn a_reply_streams_as_events() {
    let (base, server) = serve_once(concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "event: message_start\ndata: {\"type\": \"message_start\", \"message\": {\"role\": \"assistant\", \"content\": []}}\n\n",
        "event: delta\ndata: {\"type\": \"delta\", \"kind\": \"text\", \"index\": 0, \"delta\": \"Take it\"}\n\n",
        "event: done\ndata: {\"type\": \"done\"}\n\n",
    ));
    let mut events = Vec::new();
    Client::new(base).send("c1", "What now?", &json!({"day": 5}), None, &StopHandle::default(), |event| events.push(event)).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[1], AdvisorEvent::Delta { kind: DeltaKind::Text, index: 0, delta: "Take it".into() });
    assert_eq!(events[2], AdvisorEvent::Done);
    let asked = server.join().unwrap();
    assert!(asked.starts_with("POST /api/conversations/c1/messages "));
    let body: Value = serde_json::from_str(asked.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body, json!({"text": "What now?", "situation": {"day": 5}}));
}

/// The body of a chunked answer, cut into chunks of `size` bytes.
fn chunked(body: &str, size: usize) -> String {
    let mut out = String::new();
    for piece in body.as_bytes().chunks(size) {
        out.push_str(&format!("{:x}\r\n{}\r\n", piece.len(), String::from_utf8_lossy(piece)));
    }
    out + "0\r\n\r\n"
}

#[test]
fn a_chunked_reply_is_read_whatever_its_chunks() {
    let frames = concat!(
        "event: delta\ndata: {\"type\": \"delta\", \"kind\": \"text\", \"index\": 0, \"delta\": \"Take it\"}\n\n",
        "event: done\ndata: {\"type\": \"done\"}\n\n",
    );
    let (base, server) = serve_once(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n{}", chunked(frames, 7)));
    let mut events = Vec::new();
    Client::new(base).send("c1", "Hi", &json!({}), None, &StopHandle::default(), |event| events.push(event)).unwrap();
    assert_eq!(events, [AdvisorEvent::Delta { kind: DeltaKind::Text, index: 0, delta: "Take it".into() }, AdvisorEvent::Done]);
    server.join().unwrap();
}

#[test]
fn a_refused_message_carries_its_status_and_detail() {
    let (base, server) = serve_once(json_answer("409 Conflict", r#"{"detail": "A reply is still running."}"#));
    let error = Client::new(base).send("c1", "Hi", &json!({}), None, &StopHandle::default(), |_| {}).unwrap_err();
    assert_eq!(error, ClientError::Status { status: 409, detail: "A reply is still running.".into() });
    server.join().unwrap();
}

#[test]
fn stop_closes_a_reply_that_is_waiting_and_ends_it_without_an_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    // A backend that sends the head and one event, then waits on the model until the stream closes.
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 4096];
        let mut asked = String::new();
        while !asked.split_once("\r\n\r\n").is_some_and(|(_, body)| body.ends_with('}')) {
            let read = stream.read(&mut buffer).unwrap();
            asked.push_str(&String::from_utf8_lossy(&buffer[..read]));
        }
        let event = "event: delta\ndata: {\"type\": \"delta\", \"kind\": \"text\", \"index\": 0, \"delta\": \"Take\"}\n\n";
        stream.write_all(format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{event}\r\n", event.len()).as_bytes()).unwrap();
        // Reading again returns once the client has closed its side.
        stream.read(&mut buffer).unwrap_or(0)
    });
    let stop = StopHandle::default();
    let (events, done) = std::sync::mpsc::channel();
    let streaming = {
        let stop = stop.clone();
        std::thread::spawn(move || Client::new(base).send("c1", "Hi", &json!({}), None, &stop, |event| events.send(event).unwrap()))
    };
    // The first event has come: the reply is under way.
    assert!(matches!(done.recv_timeout(Duration::from_secs(5)).unwrap(), AdvisorEvent::Delta { .. }));
    stop.stop();
    assert_eq!(streaming.join().unwrap(), Ok(()));
    assert_eq!(server.join().unwrap(), 0, "the backend sees the stream close");
}

#[test]
fn stop_before_the_stream_opens_sends_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = StopHandle::default();
    stop.stop();
    assert_eq!(Client::new(base).send("c1", "Hi", &json!({}), None, &stop, |_| panic!("no events")), Ok(()));
}

#[test]
fn a_head_says_how_its_body_ends() {
    let mut chunked_head = std::io::Cursor::new("HTTP/1.1 200 OK\r\ntransfer-encoding: Chunked\r\nX: y\r\n\r\nrest");
    assert_eq!(read_head(&mut chunked_head).unwrap(), Head { status: 200, chunked: true, length: None });
    let mut sized = std::io::Cursor::new("HTTP/1.1 404 Not Found\r\nContent-Length: 12\r\n\r\n");
    assert_eq!(read_head(&mut sized).unwrap(), Head { status: 404, chunked: false, length: Some(12) });
    assert!(read_head(&mut std::io::Cursor::new("")).is_err());
    assert!(read_head(&mut std::io::Cursor::new("garbage\r\n\r\n")).is_err());
}

#[test]
fn a_chunked_body_reads_as_its_bytes() {
    let mut body = String::new();
    Chunked::new(std::io::Cursor::new("4;ext=1\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\nTrailer: x\r\n\r\n")).read_to_string(&mut body).unwrap();
    assert_eq!(body, "Wikipedia in\r\n\r\nchunks.");
    assert!(Chunked::new(std::io::Cursor::new("zz\r\n")).read_to_string(&mut String::new()).is_err());
}

#[test]
fn nothing_listening_is_unreachable() {
    // A port that was free a moment ago, and is closed again.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let error = Client::new(format!("http://127.0.0.1:{port}")).conversations("r1").unwrap_err();
    assert_eq!(error, ClientError::Unreachable);
}

#[test]
fn path_segments_are_encoded() {
    assert_eq!(encode("local-1a_B.~"), "local-1a_B.~");
    assert_eq!(encode("a/b c?"), "a%2Fb%20c%3F");
}

#[test]
fn a_message_carries_the_chosen_model() {
    let (base, server) = serve_once("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: done\ndata: {\"type\": \"done\"}\n\n");
    Client::new(base).send("c1", "Hi", &json!({}), Some("vendor/model:free"), &StopHandle::default(), |_| {}).unwrap();
    let asked = server.join().unwrap();
    let body: Value = serde_json::from_str(asked.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body, json!({"text": "Hi", "situation": {}, "model": "vendor/model:free"}));
    // No model chosen: none is sent, so the backend's ADVISOR_MODEL answers.
    assert_eq!(message_body("Hi", &json!({}), None), json!({"text": "Hi", "situation": {}}));
}

#[test]
fn the_models_are_listed() {
    let (base, server) = serve_once(json_answer("200 OK", r#"{"default": "a/b", "models": [{"id": "a/b", "name": "A B"}, {"id": "c/d"}], "source": "endpoint"}"#));
    let list = Client::new(base).models().unwrap();
    assert_eq!(list.default, "a/b");
    assert_eq!(list.models, vec![ModelOption { id: "a/b".into(), name: "A B".into(), ..Default::default() }, ModelOption { id: "c/d".into(), name: String::new(), ..Default::default() }]);
    assert!(server.join().unwrap().starts_with("GET /api/models "));
}

#[test]
fn a_model_carries_its_prices_per_million_tokens() {
    let list: ModelList = serde_json::from_str(
        r#"{"default": "a/b", "models": [{"id": "a/b", "name": "A B", "inputUsdPerMillion": 3.0, "outputUsdPerMillion": 15.0}, {"id": "c/d"}]}"#,
    )
    .unwrap();
    assert_eq!(list.models[0].input_usd_per_million, Some(3.0));
    assert_eq!(list.models[0].output_usd_per_million, Some(15.0));
    assert_eq!(list.models[1].input_usd_per_million, None);
}

#[test]
fn a_price_label_is_as_short_as_it_can_be() {
    let priced = |input, output| ModelOption { input_usd_per_million: input, output_usd_per_million: output, ..Default::default() };
    assert_eq!(priced(Some(3.0), Some(15.0)).price_label().as_deref(), Some("$3 in · $15 out /M"));
    assert_eq!(priced(Some(0.075), Some(0.3)).price_label().as_deref(), Some("$0.075 in · $0.3 out /M"));
    assert_eq!(priced(Some(0.0), Some(0.0)).price_label().as_deref(), Some("free"));
    assert_eq!(priced(None, Some(1.0)).price_label(), None);
}
