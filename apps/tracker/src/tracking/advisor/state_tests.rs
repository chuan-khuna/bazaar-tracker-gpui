use serde_json::json;

use super::*;
use crate::tracking::advisor::client::SavedMessage;

fn conversation(id: &str, run_id: &str, created_ms: i64) -> Conversation {
    Conversation { id: id.into(), run_id: run_id.into(), title: format!("About {id}"), day: Some(3), hour: Some(1), created_ms }
}

fn list(run_id: &str) -> Vec<Conversation> {
    vec![conversation("old", run_id, 1), conversation("new", run_id, 3), conversation("mid", run_id, 2)]
}

fn messages(id: &str) -> Request {
    Request::Messages { conversation_id: id.into() }
}

#[test]
fn a_rows_icon_opens_its_run_and_the_main_icon_the_live_run() {
    assert_eq!(icon_run(ChatIcon::Row("r1"), Some("live")), Some("r1"));
    assert_eq!(icon_run(ChatIcon::Main, Some("live")), Some("live"));
    // Without a Live run the Main island's icon is greyed out.
    assert_eq!(icon_run(ChatIcon::Main, None), None);
}

#[test]
fn an_icon_opens_the_runs_newest_conversation() {
    let mut state = AdvisorState::default();
    assert_eq!(state.open("r1", true), [Request::List { run_id: "r1".into() }]);
    assert_eq!(state.on_conversations("r1", Ok(list("r1"))), [messages("new")]);
    assert_eq!(state.selected(), Some("new"));
    assert_eq!(state.conversations().iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["new", "mid", "old"]);
}

#[test]
fn an_icon_goes_back_to_the_newest_after_another_was_chosen() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    assert_eq!(state.select("old"), [messages("old")]);
    // Selecting the Run again from the sidebar keeps the one chosen.
    assert!(state.open("r1", false).is_empty());
    assert_eq!(state.selected(), Some("old"));
    // Its chat icon asks again and shows the newest.
    assert_eq!(state.open("r1", true), [Request::List { run_id: "r1".into() }]);
    assert_eq!(state.on_conversations("r1", Ok(list("r1"))), [messages("new")]);
}

#[test]
fn a_run_without_conversations_shows_none_until_the_first_message() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    assert!(state.on_conversations("r1", Ok(vec![])).is_empty());
    assert_eq!(state.selected(), None);
    state.input = "  What now?\n".into();
    assert_eq!(state.send(json!({"day": 3})), [Request::Create { run_id: "r1".into(), text: "What now?".into(), situation: json!({"day": 3}) }]);
    assert!(state.input.is_empty());
    assert!(state.busy());
    let made = conversation("c1", "r1", 5);
    assert_eq!(state.on_created("r1", "What now?".into(), json!({"day": 3}), Ok(made)), [Request::Send { conversation_id: "c1".into(), text: "What now?".into(), situation: json!({"day": 3}) }]);
    assert_eq!(state.selected(), Some("c1"));
    assert_eq!(state.conversations()[0].id, "c1");
    assert!(state.replying());
}

#[test]
fn a_message_goes_to_the_shown_conversation() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    state.input = "And now?".into();
    assert_eq!(state.send(json!({})), [Request::Send { conversation_id: "new".into(), text: "And now?".into(), situation: json!({}) }]);
    // A second message waits for the reply.
    state.input = "Hello?".into();
    assert!(state.send(json!({})).is_empty());
    state.on_event("new", &AdvisorEvent::MessageStart(json!({"role": "user", "content": "And now?"})));
    state.on_ended("new", Ok(()));
    assert!(!state.busy());
    assert_eq!(state.transcript().entries.len(), 1);
}

#[test]
fn another_run_starts_at_its_newest_and_the_tab_keeps_the_draft() {
    let mut state = AdvisorState::default();
    state.open("r1", false);
    state.on_conversations("r1", Ok(list("r1")));
    state.select("old");
    state.input = "draft".into();
    assert_eq!(state.open("r2", false), [Request::List { run_id: "r2".into() }]);
    assert_eq!(state.selected(), None);
    assert!(state.conversations().is_empty());
    // A late answer for the first Run is passed over.
    assert!(state.on_conversations("r1", Ok(list("r1"))).is_empty());
    assert_eq!(state.on_conversations("r2", Ok(vec![conversation("x", "r2", 1)])), [messages("x")]);
    assert_eq!(state.input, "draft");
}

#[test]
fn saved_messages_fill_the_shown_conversation() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    let saved = Saved { running: true, messages: vec![SavedMessage { seq: 0, message: json!({"role": "user", "content": "Hi"}) }] };
    state.on_messages("mid", Ok(saved.clone()));
    assert!(state.transcript().entries.is_empty());
    state.on_messages("new", Ok(saved));
    assert_eq!(state.transcript().entries[0].text, "Hi");
    // A reply running from elsewhere: nothing more is sent to it until it ends.
    assert!(state.replying());
    state.input = "More".into();
    assert!(state.send(json!({})).is_empty());
}

#[test]
fn when_nothing_answers_the_tab_says_how_to_start_the_advisor_and_tries_again() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Err(ClientError::Unreachable));
    assert_eq!(state.status().as_deref(), Some(NOT_RUNNING));
    assert!(NOT_RUNNING.contains("just advisor serve"));
    // Opening the tab again asks again.
    assert_eq!(state.open("r1", false), [Request::List { run_id: "r1".into() }]);
    // So does sending: the message goes back in the box if it didn't get there.
    state.input = "Hi".into();
    let sent = state.send(json!({}));
    assert!(matches!(sent.as_slice(), [Request::Create { .. }]));
    state.on_created("r1", "Hi".into(), json!({}), Err(ClientError::Unreachable));
    assert_eq!(state.input, "Hi");
    assert!(!state.busy());
    state.on_conversations("r1", Ok(vec![]));
    assert_eq!(state.status(), None);
}

#[test]
fn an_unusable_history_db_is_said_as_the_backend_says_it() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Err(ClientError::Status { status: 503, detail: "history.db is at schema version 8".into() }));
    assert_eq!(state.status().as_deref(), Some("history.db is at schema version 8"));
}

#[test]
fn deleting_asks_first_and_then_shows_the_newest_left() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    assert!(state.delete("new").is_empty());
    assert_eq!(state.deleting(), Some("new"));
    assert_eq!(state.delete("new"), [Request::Delete { conversation_id: "new".into() }]);
    assert_eq!(state.on_deleted("new", Ok(())), [messages("mid")]);
    assert_eq!(state.conversations().len(), 2);
}

#[test]
fn a_rebound_run_keeps_its_conversations_and_a_deleted_one_forgets_them() {
    let mut state = AdvisorState::default();
    state.open("local-1", true);
    state.on_conversations("local-1", Ok(list("local-1")));
    state.rebound("local-1", "real");
    assert_eq!(state.run_id(), Some("real"));
    assert!(state.open("real", false).is_empty());
    state.forget(Some("other"));
    assert_eq!(state.run_id(), Some("real"));
    state.forget(Some("real"));
    assert_eq!(state.run_id(), None);
    assert!(state.conversations().is_empty());
}
