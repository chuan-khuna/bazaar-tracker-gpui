use serde_json::json;

use super::*;

fn frame(event: &str, data: &str) -> Frame {
    Frame { event: event.into(), data: data.into() }
}

#[test]
fn a_frame_ends_at_a_blank_line() {
    let mut parser = SseParser::default();
    assert_eq!(parser.feed("event: done\ndata: {\"type\": \"done\"}\n\n"), [frame("done", "{\"type\": \"done\"}")]);
}

#[test]
fn lines_cut_into_pieces_are_joined() {
    let mut parser = SseParser::default();
    assert!(parser.feed("event: del").is_empty());
    assert!(parser.feed("ta\ndata: {\"a\":").is_empty());
    assert!(parser.feed(" 1}\n").is_empty());
    assert_eq!(parser.feed("\n"), [frame("delta", "{\"a\": 1}")]);
}

#[test]
fn crlf_line_endings_are_taken() {
    let mut parser = SseParser::default();
    assert_eq!(parser.feed("event: done\r\ndata: {}\r\n\r\n"), [frame("done", "{}")]);
}

#[test]
fn several_data_lines_are_joined_with_newlines() {
    let mut parser = SseParser::default();
    assert_eq!(parser.feed("data: one\ndata: two\ndata:three\n\n"), [frame("", "one\ntwo\nthree")]);
}

#[test]
fn comments_and_other_fields_are_passed_over() {
    let mut parser = SseParser::default();
    assert_eq!(parser.feed(": keep-alive\nid: 7\nretry: 100\nevent: done\ndata: {}\n\n"), [frame("done", "{}")]);
}

#[test]
fn a_blank_line_without_data_is_no_frame_and_forgets_the_event() {
    let mut parser = SseParser::default();
    assert!(parser.feed("event: stray\n\n").is_empty());
    assert_eq!(parser.feed("data: {}\n\n"), [frame("", "{}")]);
}

#[test]
fn two_frames_in_one_piece_come_in_order() {
    let mut parser = SseParser::default();
    let frames = parser.feed("event: a\ndata: 1\n\nevent: b\ndata: 2\n\n");
    assert_eq!(frames, [frame("a", "1"), frame("b", "2")]);
}

#[test]
fn an_open_frame_counts_when_the_stream_ends() {
    let mut parser = SseParser::default();
    assert!(parser.feed("event: done\ndata: {}").is_empty());
    assert_eq!(parser.finish(), Some(frame("done", "{}")));
    assert_eq!(parser.finish(), None);
}

#[test]
fn events_are_read_from_their_frames() {
    let text = AdvisorEvent::from_frame(&frame("delta", r#"{"type": "delta", "kind": "text", "index": 0, "delta": "Take the"}"#));
    assert_eq!(text, AdvisorEvent::Delta { kind: DeltaKind::Text, index: 0, delta: "Take the".into() });
    let thinking = AdvisorEvent::from_frame(&frame("delta", r#"{"kind": "thinking", "index": 1, "delta": "hm"}"#));
    assert_eq!(thinking, AdvisorEvent::Delta { kind: DeltaKind::Thinking, index: 1, delta: "hm".into() });
    let start = AdvisorEvent::from_frame(&frame("message_start", r#"{"type": "message_start", "message": {"role": "user", "content": "Hi"}}"#));
    assert_eq!(start, AdvisorEvent::MessageStart(json!({"role": "user", "content": "Hi"})));
    assert_eq!(AdvisorEvent::from_frame(&frame("error", r#"{"type": "error", "error": "boom"}"#)), AdvisorEvent::Error("boom".into()));
    assert_eq!(AdvisorEvent::from_frame(&frame("done", r#"{"type": "done"}"#)), AdvisorEvent::Done);
    let stats = AdvisorEvent::from_frame(&frame("stats", r#"{"type": "stats", "seq": 3, "stats": {"model": "m"}}"#));
    assert_eq!(stats, AdvisorEvent::Stats(json!({"type": "stats", "seq": 3, "stats": {"model": "m"}})));
}

#[test]
fn without_an_event_line_the_name_comes_from_the_type() {
    assert_eq!(AdvisorEvent::from_frame(&frame("", r#"{"type": "done"}"#)), AdvisorEvent::Done);
}

#[test]
fn unknown_events_and_bad_data_are_unknown() {
    assert_eq!(AdvisorEvent::from_frame(&frame("ping", r#"{"type": "ping"}"#)), AdvisorEvent::Unknown("ping".into()));
    assert_eq!(AdvisorEvent::from_frame(&frame("delta", "not json")), AdvisorEvent::Unknown("delta".into()));
    let other = AdvisorEvent::from_frame(&frame("delta", r#"{"kind": "image", "delta": "x"}"#));
    assert_eq!(other, AdvisorEvent::Delta { kind: DeltaKind::Other("image".into()), index: 0, delta: "x".into() });
}
