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
    Client::new(base).send("c1", "What now?", &json!({"day": 5}), |event| events.push(event)).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[1], AdvisorEvent::Delta { kind: DeltaKind::Text, index: 0, delta: "Take it".into() });
    assert_eq!(events[2], AdvisorEvent::Done);
    let asked = server.join().unwrap();
    assert!(asked.starts_with("POST /api/conversations/c1/messages "));
    let body: Value = serde_json::from_str(asked.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body, json!({"text": "What now?", "situation": {"day": 5}}));
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
