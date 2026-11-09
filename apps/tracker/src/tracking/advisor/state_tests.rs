use serde_json::json;

use super::*;
use std::time::Instant;

use crate::tracking::advisor::client::SavedMessage;
use crate::tracking::advisor::reply::{End, Part, Question, Turn};
use crate::tracking::advisor::sse::DeltaKind;

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
    assert_eq!(state.send(json!({"day": 3})), [Request::Create { run_id: "r1".into(), text: "What now?".into(), situation: json!({"day": 3}), model: None }]);
    assert!(state.input.is_empty());
    assert!(state.busy());
    let made = conversation("c1", "r1", 5);
    assert_eq!(state.on_created("r1", "What now?".into(), json!({"day": 3}), None, Ok(made)), [Request::Send { conversation_id: "c1".into(), text: "What now?".into(), situation: json!({"day": 3}), model: None }]);
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
    assert_eq!(state.send(json!({})), [Request::Send { conversation_id: "new".into(), text: "And now?".into(), situation: json!({}), model: None }]);
    // A second message waits for the reply.
    state.input = "Hello?".into();
    assert!(state.send(json!({})).is_empty());
    state.on_event("new", &AdvisorEvent::MessageStart(json!({"role": "user", "content": "And now?"})), Instant::now());
    state.on_ended("new", Ok(()), Instant::now());
    assert!(!state.busy());
    // The streamed question carries the situation it was sent with, for its Context row.
    assert_eq!(state.transcript().turns, [Turn::User(Question { text: "And now?".into(), situation: Some(json!({})) })]);
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
    let saved = Saved { running: true, messages: vec![SavedMessage { seq: 0, message: json!({"role": "user", "content": "Hi"}), stats: None, situation: None }] };
    state.on_messages("mid", Ok(saved.clone()));
    assert!(state.transcript().is_empty());
    state.on_messages("new", Ok(saved));
    assert_eq!(state.transcript().turns, [Turn::User("Hi".into())]);
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
    // The command follows as inline code, and its copy button copies exactly this.
    assert_eq!(state.status_command(), Some("just advisor serve"));
    assert!(!NOT_RUNNING.contains('`'));
    // Opening the tab again asks again.
    assert_eq!(state.open("r1", false), [Request::List { run_id: "r1".into() }]);
    // So does sending: the message goes back in the box if it didn't get there.
    state.input = "Hi".into();
    let sent = state.send(json!({}));
    assert!(matches!(sent.as_slice(), [Request::Create { .. }]));
    state.on_created("r1", "Hi".into(), json!({}), None, Err(ClientError::Unreachable));
    assert_eq!(state.input, "Hi");
    assert!(!state.busy());
    state.on_conversations("r1", Ok(vec![]));
    assert_eq!(state.status(), None);
    assert_eq!(state.status_command(), None);
}

#[test]
fn an_unusable_history_db_is_said_as_the_backend_says_it() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Err(ClientError::Status { status: 503, detail: "history.db is at schema version 8".into() }));
    assert_eq!(state.status().as_deref(), Some("history.db is at schema version 8"));
    assert_eq!(state.status_command(), None);
}

#[test]
fn deleting_asks_first_and_then_shows_the_newest_left() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    // Delete does nothing until the strip asks.
    assert!(state.delete("new").is_empty());
    state.ask_delete("new");
    assert_eq!(state.deleting(), Some("new"));
    // Cancel (or Escape) closes it.
    assert!(state.cancel_delete());
    assert!(!state.cancel_delete());
    state.ask_delete("new");
    assert_eq!(state.delete("new"), [Request::Delete { conversation_id: "new".into() }]);
    assert_eq!(state.on_deleted("new", Ok(())), [messages("mid")]);
    assert_eq!(state.conversations().len(), 2);
}

#[test]
fn clicking_another_conversation_closes_the_delete_strip() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    state.ask_delete("new");
    state.select("new");
    assert_eq!(state.deleting(), Some("new"));
    state.select("mid");
    assert_eq!(state.deleting(), None);
    assert!(state.delete("new").is_empty());
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

/// A state showing Conversation "new" of Run "r1", with `messages` saved in it.
fn showing(messages: Vec<serde_json::Value>) -> AdvisorState {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    let messages = messages.into_iter().enumerate().map(|(seq, message)| SavedMessage { seq: seq as i64, message, stats: None, situation: None }).collect();
    state.on_messages("new", Ok(Saved { running: false, messages }));
    state
}

fn send(id: &str, text: &str) -> Request {
    Request::Send { conversation_id: id.into(), text: text.into(), situation: json!({}), model: None }
}

#[test]
fn send_becomes_stop_and_stop_closes_the_stream_keeping_the_reply() {
    let mut state = showing(vec![]);
    let now = Instant::now();
    assert!(!state.can_stop());
    state.input = "Which one?".into();
    state.send(json!({}));
    assert!(state.can_stop());
    state.on_event("new", &AdvisorEvent::MessageStart(json!({"role": "user", "content": "Which one?"})), now);
    state.on_event("new", &AdvisorEvent::MessageStart(json!({"role": "assistant", "content": []})), now);
    state.on_event("new", &AdvisorEvent::Delta { kind: DeltaKind::Text, index: 0, delta: "Take".into() }, now);
    assert_eq!(state.stop(now), [Request::Stop { conversation_id: "new".into() }]);
    assert!(!state.can_stop());
    // What still comes before the stream closes is passed over.
    state.on_event("new", &AdvisorEvent::Delta { kind: DeltaKind::Text, index: 0, delta: " the ball".into() }, now);
    let Some(Turn::Reply(reply)) = state.transcript().turns.last() else { panic!("no reply") };
    assert_eq!(reply.end, End::Stopped);
    assert_eq!(reply.steps[0], [Part::Text("Take".into())]);
    // Nothing more is sent until the stream has closed.
    assert!(state.busy());
    state.on_ended("new", Ok(()), now);
    assert!(!state.busy());
    let Some(Turn::Reply(reply)) = state.transcript().turns.last() else { panic!("no reply") };
    assert_eq!(reply.end, End::Stopped);
    state.input = "Again".into();
    assert_eq!(state.send(json!({})), [send("new", "Again")]);
}

#[test]
fn stop_while_the_conversation_is_made_sends_nothing() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(vec![]));
    state.input = "Hi".into();
    state.send(json!({}));
    assert!(state.can_stop());
    assert!(state.stop(Instant::now()).is_empty());
    assert!(!state.can_stop());
    assert!(state.on_created("r1", "Hi".into(), json!({}), None, Ok(conversation("c1", "r1", 9))).is_empty());
    assert_eq!(state.selected(), Some("c1"));
    assert_eq!(state.input, "Hi");
    assert!(!state.busy());
}

#[test]
fn a_stream_that_breaks_fails_the_reply_and_a_failed_send_gives_the_text_back() {
    let mut state = showing(vec![]);
    let now = Instant::now();
    state.input = "Hi".into();
    state.send(json!({}));
    state.on_event("new", &AdvisorEvent::MessageStart(json!({"role": "user", "content": "Hi"})), now);
    state.on_event("new", &AdvisorEvent::MessageStart(json!({"role": "assistant", "content": []})), now);
    state.on_ended("new", Err(ClientError::Other("the reply stopped: reset".into())), now);
    let Some(Turn::Reply(reply)) = state.transcript().turns.last() else { panic!("no reply") };
    assert_eq!(reply.end, End::Failed("the reply stopped: reset".into()));
    // Shown on the reply, not above the messages.
    assert_eq!(state.status(), None);

    state.input = "Again".into();
    state.send(json!({}));
    state.on_ended("new", Err(ClientError::Status { status: 409, detail: "A reply is still running.".into() }), now);
    assert_eq!(state.input, "Again");
    assert_eq!(state.status().as_deref(), Some("A reply is still running."));
}

#[test]
fn a_follow_up_button_sends_its_question() {
    let mut state = showing(vec![]);
    state.input = "draft".into();
    assert_eq!(state.follow_up(" Why not the Wheel? ", json!({})), [send("new", "Why not the Wheel?")]);
    // The draft in the box is left alone, and nothing more goes while the reply streams.
    assert_eq!(state.input, "draft");
    assert!(state.follow_up("Another?", json!({})).is_empty());
}

fn ask_messages(multiple: bool) -> Vec<serde_json::Value> {
    vec![
        json!({"role": "user", "content": "Help"}),
        json!({"role": "assistant", "content": [{"type": "toolCall", "id": "q1", "name": "ask_user", "arguments": {"question": "Which plan?", "options": [{"id": "a", "label": "Weapons"}, {"id": "b", "label": "Shields"}], "multiple": multiple}}], "stopReason": "toolUse"}),
        json!({"role": "toolResult", "toolCallId": "q1", "toolName": "ask_user", "content": [], "details": {"status": "waiting_for_user"}}),
    ]
}

#[test]
fn the_ask_user_card_sends_its_answer_in_tau_apps_format() {
    let mut state = showing(ask_messages(true));
    state.ask_pick("q1", "b");
    state.ask_pick("q1", "a");
    state.set_ask_text("and gold".into());
    assert_eq!(state.ask_text(), "and gold");
    assert_eq!(state.answer("q1", json!({})), [send("new", "- Weapons\n- Shields\nand gold")]);
    assert!(state.draft().picked.is_empty());

    let mut state = showing(ask_messages(false));
    state.ask_pick("q1", "a");
    state.ask_pick("q1", "b");
    assert_eq!(state.answer("q1", json!({})), [send("new", "Shields")]);
}

#[test]
fn an_answered_card_is_locked() {
    let mut messages = ask_messages(false);
    messages.push(json!({"role": "user", "content": "Shields"}));
    let mut state = showing(messages);
    state.ask_pick("q1", "a");
    assert!(state.draft().picked.is_empty());
    assert!(state.answer("q1", json!({})).is_empty());
    // An unknown card, or nothing picked, sends nothing either.
    let mut state = showing(ask_messages(false));
    assert!(state.answer("q1", json!({})).is_empty());
    assert!(state.answer("other", json!({})).is_empty());
}

#[test]
fn thinking_and_tool_lines_open_and_close() {
    let mut state = showing(vec![]);
    assert!(!state.is_open("1.0.0"));
    state.toggle("1.0.0");
    assert!(state.is_open("1.0.0"));
    state.toggle("1.0.0");
    assert!(!state.is_open("1.0.0"));
    // Another Conversation starts with all closed.
    state.toggle("1.0.0");
    state.select("mid");
    assert!(!state.is_open("1.0.0"));
}

#[test]
fn a_double_click_closes_an_open_block_and_leaves_a_closed_one_closed() {
    let mut state = showing(vec![]);
    state.toggle("1.0.0");
    assert!(state.collapse("1.0.0"));
    assert!(!state.is_open("1.0.0"));
    // On a closed one it does nothing: it doesn't open it.
    assert!(!state.collapse("1.0.0"));
    assert!(!state.is_open("1.0.0"));
    // Only that block closes.
    state.toggle("1.0.0");
    state.toggle("1.0.1");
    state.collapse("1.0.1");
    assert!(state.is_open("1.0.0"));
}

#[test]
fn every_message_carries_the_model_chosen_when_it_was_sent() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(Vec::new()));
    state.choose_model(Some(" vendor/a "));
    state.input = "What now?".into();
    assert_eq!(state.send(json!({})), [Request::Create { run_id: "r1".into(), text: "What now?".into(), situation: json!({}), model: Some("vendor/a".into()) }]);
    // Changing the picker while the Conversation is made doesn't change the message on its way.
    state.choose_model(Some("vendor/b"));
    let sent = state.on_created("r1", "What now?".into(), json!({}), Some("vendor/a".into()), Ok(conversation("c1", "r1", 9)));
    assert_eq!(sent, [Request::Send { conversation_id: "c1".into(), text: "What now?".into(), situation: json!({}), model: Some("vendor/a".into()) }]);
    state.on_ended("c1", Ok(()), Instant::now());
    state.input = "And now?".into();
    assert_eq!(state.send(json!({})), [Request::Send { conversation_id: "c1".into(), text: "And now?".into(), situation: json!({}), model: Some("vendor/b".into()) }]);
}

#[test]
fn the_picker_shows_the_chosen_model_without_the_backend_and_keeps_it_across_runs() {
    let mut state = AdvisorState::default();
    assert_eq!(state.model_label(), BACKEND_MODEL);
    state.choose_model(Some("vendor/saved"));
    state.on_models(Err(ClientError::Unreachable));
    assert_eq!(state.model_label(), "vendor/saved");
    assert!(state.models().is_empty());
    state.open("r1", true);
    state.open("r2", true);
    state.forget(None);
    assert_eq!(state.model(), Some("vendor/saved"));
    assert_eq!(state.open_picker(), [Request::Models]);

    // No model chosen: the backend's default once it says it. Blank is no model.
    state.choose_model(Some("  "));
    assert_eq!(state.model(), None);
    let list = ModelList { default: "vendor/env".into(), models: vec![ModelOption { id: "vendor/env".into(), name: "Env".into(), ..Default::default() }] };
    state.on_models(Ok(list.clone()));
    assert_eq!(state.model_label(), "vendor/env");
    assert_eq!(state.backend_model(), Some("vendor/env"));
    // A failure later keeps the list.
    state.on_models(Err(ClientError::Unreachable));
    assert_eq!(state.models(), list.models.as_slice());
}

#[test]
fn the_conversation_list_folds_and_stays_folded_across_runs() {
    let mut state = AdvisorState::default();
    assert!(!state.list_collapsed());
    state.toggle_list();
    assert!(state.list_collapsed());
    state.open("r1", true);
    state.open("r2", false);
    state.forget(None);
    assert!(state.list_collapsed());
    state.toggle_list();
    assert!(!state.list_collapsed());
}

#[test]
fn the_folded_header_names_the_shown_conversation_and_the_count() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    // No Conversations: nothing beside the title, open or folded.
    assert_eq!(state.list_summary(), None);
    state.toggle_list();
    assert_eq!(state.list_summary(), None);
    state.toggle_list();

    state.on_conversations("r1", Ok(list("r1")));
    assert_eq!(state.list_summary().as_deref(), Some("3"));
    state.toggle_list();
    assert_eq!(state.list_summary().as_deref(), Some("3 · About new D3 H1"));
    state.select("old");
    assert_eq!(state.list_summary().as_deref(), Some("3 · About old D3 H1"));
    // `+` while folded: a new one is being started.
    state.new_conversation();
    assert_eq!(state.list_summary().as_deref(), Some("3 · New conversation"));
    assert!(state.list_collapsed());
}

#[test]
fn the_open_list_scrolls_past_five_rows_to_the_shown_conversation() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    assert_eq!(state.scroll_target(), Some((0, "new")));
    state.select("old");
    assert_eq!(state.scroll_target(), Some((2, "old")));
    state.toggle_list();
    assert_eq!(state.scroll_target(), None);
    state.toggle_list();

    let many: Vec<_> = (0..=LIST_ROWS as i64).map(|n| conversation(&format!("c{n}"), "r1", n)).collect();
    state.on_conversations("r1", Ok(many));
    // The shown one went: the newest is shown, at the top.
    assert_eq!(state.scroll_target(), Some((0, "c5")));
    state.select("c0");
    assert_eq!(state.scroll_target(), Some((LIST_ROWS, "c0")));
    state.toggle_list();
    state.new_conversation();
    state.toggle_list();
    assert_eq!(state.scroll_target(), None);
}

fn attachment(run_id: &str) -> Attachment {
    Attachment { run_id: run_id.into(), event_id: "e7".into(), label: "Event #7 · D5 H2 · Options: 3 options · Choice".into() }
}

#[test]
fn an_event_icon_opens_a_new_conversation_with_the_event_attached() {
    let mut state = AdvisorState::default();
    state.open("r1", true);
    state.on_conversations("r1", Ok(list("r1")));
    assert_eq!(state.selected(), Some("new"));
    // Another Run's Event: its list is read, and no Conversation is shown, even once it comes.
    assert_eq!(state.open_event(attachment("r2")), [Request::List { run_id: "r2".into() }]);
    assert!(state.on_conversations("r2", Ok(list("r2"))).is_empty());
    assert_eq!(state.selected(), None);
    assert_eq!(state.attached(), Some(&attachment("r2")));
    // The same Run's Event, with a Conversation shown: a new one, without asking again.
    state.select("old");
    assert!(state.open_event(attachment("r2")).is_empty());
    assert_eq!(state.selected(), None);
}

#[test]
fn the_attachment_goes_with_one_message_only() {
    let mut state = AdvisorState::default();
    state.open_event(attachment("r1"));
    state.on_conversations("r1", Ok(vec![]));
    state.input = "Was this right?".into();
    let at_event = json!({"day": 5, "hour": 2, "event": {"id": "e7", "number": 7, "kind": "Options"}});
    assert_eq!(state.send(at_event.clone()), [Request::Create { run_id: "r1".into(), text: "Was this right?".into(), situation: at_event.clone(), model: None }]);
    assert_eq!(state.attached(), None);
    state.on_created("r1", "Was this right?".into(), at_event, None, Ok(conversation("c1", "r1", 5)));
    state.on_ended("c1", Ok(()), Instant::now());
    // A failed message gives its Event back with its text.
    state.input = "And now?".into();
    state.send(json!({}));
    state.on_ended("c1", Err(ClientError::Unreachable), Instant::now());
    assert_eq!((state.input.as_str(), state.attached()), ("And now?", None));
}

#[test]
fn a_message_that_never_arrived_puts_its_event_back_and_the_chip_can_be_removed() {
    let mut state = AdvisorState::default();
    state.open_event(attachment("r1"));
    state.on_conversations("r1", Ok(vec![]));
    state.input = "Was this right?".into();
    state.send(json!({}));
    state.on_created("r1", "Was this right?".into(), json!({}), None, Err(ClientError::Unreachable));
    assert_eq!(state.input, "Was this right?");
    assert_eq!(state.attached(), Some(&attachment("r1")));
    state.detach();
    assert_eq!(state.attached(), None);
}
