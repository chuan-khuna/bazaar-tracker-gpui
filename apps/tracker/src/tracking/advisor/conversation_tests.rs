use serde_json::json;

use super::*;

fn conversation(id: &str, created_ms: i64, day: Option<i32>, hour: Option<i32>) -> Conversation {
    Conversation { id: id.into(), run_id: "r1".into(), title: "Which item should I buy?".into(), day, hour, created_ms }
}

#[test]
fn a_conversation_is_read_from_the_api() {
    let read: Conversation = serde_json::from_value(json!({"id": "c1", "runId": "r1", "title": "Hi", "day": 5, "hour": null, "createdMs": 7})).unwrap();
    assert_eq!(read, Conversation { id: "c1".into(), run_id: "r1".into(), title: "Hi".into(), day: Some(5), hour: None, created_ms: 7 });
}

#[test]
fn the_title_carries_the_day_and_hour() {
    assert_eq!(conversation("c", 0, Some(5), Some(2)).label(), "Which item should I buy? D5 H2");
    assert_eq!(conversation("c", 0, Some(5), Some(0)).time().as_deref(), Some("D5 H0"));
    assert_eq!(conversation("c", 0, Some(5), None).label(), "Which item should I buy? D5");
    assert_eq!(conversation("c", 0, None, Some(2)).label(), "Which item should I buy?");
}

#[test]
fn the_list_is_newest_first() {
    let mut list = vec![conversation("old", 1, None, None), conversation("new", 3, None, None), conversation("mid", 2, None, None)];
    newest_first(&mut list);
    assert_eq!(list.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["new", "mid", "old"]);
}

#[test]
fn saved_messages_show_their_text_and_tools_but_not_tool_results() {
    let transcript = Transcript::from_saved(&[
        json!({"role": "user", "content": "What now?"}),
        json!({"role": "assistant", "content": [{"type": "thinking", "thinking": "hm"}, {"type": "toolCall", "id": "t1", "name": "rate_options", "arguments": {}}]}),
        json!({"role": "toolResult", "toolCallId": "t1", "toolName": "rate_options", "content": [{"type": "text", "text": "{}"}]}),
        json!({"role": "assistant", "content": [{"type": "text", "text": "Take "}, {"type": "text", "text": "the ball."}]}),
        json!({"role": "user", "content": [{"type": "text", "text": "Thanks"}]}),
        json!({"role": "assistant", "content": [], "errorMessage": "rate limited"}),
    ]);
    assert_eq!(
        transcript.entries,
        [
            Entry { role: Role::User, text: "What now?".into(), tools: vec![] },
            Entry { role: Role::Assistant, text: String::new(), tools: vec!["rate_options".into()] },
            Entry { role: Role::Assistant, text: "Take the ball.".into(), tools: vec![] },
            Entry { role: Role::User, text: "Thanks".into(), tools: vec![] },
            Entry { role: Role::Error, text: "rate limited".into(), tools: vec![] },
        ]
    );
}

#[test]
fn a_reply_streams_in_and_is_replaced_by_the_whole_message() {
    let mut transcript = Transcript::default();
    let text = |delta: &str| AdvisorEvent::Delta { kind: DeltaKind::Text, index: 0, delta: delta.into() };
    transcript.apply(&AdvisorEvent::MessageStart(json!({"role": "user", "content": "Hi"})));
    transcript.apply(&AdvisorEvent::MessageEnd(json!({"role": "user", "content": "Hi"})));
    transcript.apply(&AdvisorEvent::MessageStart(json!({"role": "assistant", "content": []})));
    transcript.apply(&AdvisorEvent::Delta { kind: DeltaKind::Thinking, index: 0, delta: "hidden".into() });
    transcript.apply(&text("Take"));
    transcript.apply(&text(" it"));
    assert_eq!(transcript.entries.last().unwrap().text, "Take it");
    transcript.apply(&AdvisorEvent::MessageEnd(json!({"role": "assistant", "content": [{"type": "text", "text": "Take it."}]})));
    transcript.apply(&AdvisorEvent::Done);
    assert_eq!(transcript.entries.iter().map(|entry| (entry.role, entry.text.as_str())).collect::<Vec<_>>(), [(Role::User, "Hi"), (Role::Assistant, "Take it.")]);
}

#[test]
fn an_empty_assistant_message_goes_and_errors_are_shown() {
    let mut transcript = Transcript::default();
    transcript.apply(&AdvisorEvent::MessageStart(json!({"role": "assistant", "content": []})));
    transcript.apply(&AdvisorEvent::MessageEnd(json!({"role": "assistant", "content": [], "errorMessage": "no key"})));
    transcript.apply(&AdvisorEvent::Error("the reply failed".into()));
    assert_eq!(transcript.entries.iter().map(|entry| (entry.role, entry.text.as_str())).collect::<Vec<_>>(), [(Role::Error, "no key"), (Role::Error, "the reply failed")]);
}
