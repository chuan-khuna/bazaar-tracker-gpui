use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::*;
use crate::tracking::advisor::ask_user::Choice;
use crate::tracking::advisor::client::Saved;
use crate::tracking::advisor::recommendation::RowData;

/// Feeds events at whole seconds after a start.
struct Feed {
    transcript: Transcript,
    start: Instant,
}

impl Feed {
    fn new() -> Self {
        Self { transcript: Transcript::default(), start: Instant::now() }
    }

    fn at(&self, seconds: f64) -> Instant {
        self.start + Duration::from_secs_f64(seconds)
    }

    fn event(&mut self, seconds: f64, event: AdvisorEvent) {
        let at = self.at(seconds);
        self.transcript.apply(&event, at);
    }

    fn reply(&self) -> &Reply {
        match self.transcript.turns.last() {
            Some(Turn::Reply(reply)) => reply,
            other => panic!("the last turn is not a reply: {other:?}"),
        }
    }
}

fn start(message: Value) -> AdvisorEvent {
    AdvisorEvent::MessageStart(message)
}

fn end(message: Value) -> AdvisorEvent {
    AdvisorEvent::MessageEnd(message)
}

fn delta(kind: DeltaKind, index: usize, text: &str) -> AdvisorEvent {
    AdvisorEvent::Delta { kind, index, delta: text.into() }
}

fn user(text: &str) -> Value {
    json!({"role": "user", "content": text})
}

fn assistant(content: Value) -> Value {
    json!({"role": "assistant", "content": content, "stopReason": "stop"})
}

fn call(id: &str, name: &str, arguments: Value) -> Value {
    json!({"type": "toolCall", "id": id, "name": name, "arguments": arguments})
}

fn result(id: &str, name: &str, details: Value, is_error: bool) -> Value {
    json!({"role": "toolResult", "toolCallId": id, "toolName": name, "content": [{"type": "text", "text": "seen by the model"}], "details": details, "isError": is_error})
}

fn stats_event() -> AdvisorEvent {
    AdvisorEvent::Stats(json!({"type": "stats", "seq": 7, "stats": {"model": "fake/advisor", "elapsedMs": 2500, "tokens": {"input": 10, "output": 5, "cacheRead": 0, "cacheWrite": 0, "total": 15}, "costUsd": 0, "pricingSource": "provider", "toolCalls": 3}}))
}

fn suggest_details() -> Value {
    json!({"title": "Shop · Day 5", "source": "15 Jules runs · S20 · 1 profile", "ranked": [
        {"rank": 1, "card": "Decanter", "reason": "Fits and scales", "runs_with": {"n": 4}},
        {"rank": 2, "card": "Cheese Wheel", "reason": "No data", "no_data": true},
    ], "runs": []})
}

/// The kinds of the reply's pieces, as short names, for checking the order.
fn kinds(reply: &Reply) -> Vec<String> {
    reply
        .pieces()
        .iter()
        .map(|piece| match piece {
            Piece::Thinking { .. } => "thinking".to_owned(),
            Piece::Tool { call, .. } => format!("tool {}", call.name),
            Piece::Text(text) => format!("text {text}"),
            Piece::Suggest { .. } => "suggest".to_owned(),
            Piece::AskUser { .. } => "ask_user".to_owned(),
        })
        .collect()
}

fn tool<'a>(reply: &'a Reply, name: &str) -> &'a ToolCall {
    reply
        .steps
        .iter()
        .flatten()
        .find_map(|part| match part {
            Part::Tool(call) if call.name == name => Some(call),
            _ => None,
        })
        .unwrap()
}

/// A whole reply as the fake Advisor streams it: thinking, rate_options, suggest, follow_ups, then text.
fn normal_reply(feed: &mut Feed) {
    feed.event(0.0, start(user("Which one?")));
    feed.event(0.0, end(user("Which one?")));
    feed.event(0.1, start(json!({"role": "assistant", "content": []})));
    feed.event(0.2, delta(DeltaKind::Thinking, 0, "Look at "));
    feed.event(1.0, delta(DeltaKind::Thinking, 0, "the Offer."));
    feed.event(3.4, delta(DeltaKind::ToolCall, 1, "{\"options\""));
    feed.event(3.5, end(assistant(json!([{"type": "thinking", "thinking": "Look at the Offer."}, call("t1", "rate_options", json!({"options": ["Decanter"]}))]))));
    feed.event(3.6, start(result("t1", "rate_options", json!({"n": 15}), false)));
    feed.event(3.7, end(result("t1", "rate_options", json!({"n": 15}), false)));
    feed.event(3.8, start(json!({"role": "assistant", "content": []})));
    feed.event(3.9, end(assistant(json!([call("t2", "suggest", json!({"title": "Shop · Day 5", "ranked": [{"card": "Decanter", "reason": "Fits and scales"}]})), call("t3", "follow_ups", json!({"questions": ["What next?", "Why not the Wheel?"]}))]))));
    feed.event(4.0, end(result("t2", "suggest", suggest_details(), false)));
    feed.event(4.1, end(result("t3", "follow_ups", json!({"questions": ["What next?", "Why not the Wheel?"]}), false)));
    feed.event(4.2, start(json!({"role": "assistant", "content": []})));
    feed.event(4.3, delta(DeltaKind::Text, 0, "Take **Decanter**"));
    feed.event(4.4, end(assistant(json!([{"type": "text", "text": "Take **Decanter**."}]))));
    feed.event(4.5, stats_event());
    feed.event(4.5, AdvisorEvent::Done);
}

#[test]
fn a_normal_reply_is_drawn_in_order_with_its_cards_buttons_and_usage() {
    let mut feed = Feed::new();
    normal_reply(&mut feed);
    assert_eq!(feed.transcript.turns[0], Turn::User("Which one?".into()));
    let reply = feed.reply();
    assert_eq!(reply.end, End::Done);
    assert_eq!(reply.steps.len(), 3);
    // Thinking, the tool line and the text in order; suggest as its own card after the text;
    // follow_ups not as a line but as buttons.
    assert_eq!(kinds(reply), ["thinking", "tool rate_options", "text Take **Decanter**.", "suggest"]);
    assert_eq!(reply.follow_ups(), ["What next?", "Why not the Wheel?"]);
    assert_eq!(reply.stats.as_ref().unwrap().line(), ["fake/advisor", "2.5 s", "15 tokens", "free", "3 tool calls"]);
    let Some(Piece::Suggest { card, .. }) = reply.pieces().into_iter().last() else { panic!("no suggest card") };
    // Drawn from the result: its rows best first, numbers or "no data", and the source line.
    assert_eq!(card.title, "Shop · Day 5");
    assert!(!card.pending);
    let rows: Vec<(usize, &str, &str)> = card.rows.iter().map(|row| (row.rank, row.card.as_str(), row.reason.as_str())).collect();
    assert_eq!(rows, [(1, "Decanter", "Fits and scales"), (2, "Cheese Wheel", "No data")]);
    assert!(matches!(card.rows[0].data, RowData::Bars(_)));
    assert_eq!(card.rows[1].data, RowData::NoData);
    assert_eq!(card.source.map(|source| source.text), Some("15 Jules runs · S20 · 1 profile".into()));
}

#[test]
fn thinking_is_timed_while_it_streams_then_reads_as_thought() {
    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(1.0, delta(DeltaKind::Thinking, 0, "hm"));
    let thinking = |feed: &Feed| match &feed.reply().steps[0][0] {
        Part::Thinking(thinking) => thinking.clone(),
        other => panic!("not thinking: {other:?}"),
    };
    assert_eq!(thinking(&feed).label(feed.at(3.7)), "Thinking… 2s");
    assert!(feed.reply().busy());
    // Text after it ends the thinking.
    feed.event(4.6, delta(DeltaKind::Text, 1, "Take it"));
    assert_eq!(thinking(&feed).label(feed.at(60.0)), "Thought for 4s");
    assert!(!feed.reply().busy());
    // The whole message keeps the time.
    feed.event(5.0, end(assistant(json!([{"type": "thinking", "thinking": "hm"}, {"type": "text", "text": "Take it"}]))));
    assert_eq!(thinking(&feed).label(feed.at(60.0)), "Thought for 4s");
    assert_eq!(thinking(&feed).text, "hm");
}

#[test]
fn thinking_ended_by_the_message_end_and_a_short_one_reads_one_second() {
    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(0.0, delta(DeltaKind::Thinking, 0, "quick"));
    feed.event(0.2, end(assistant(json!([{"type": "thinking", "thinking": "quick"}]))));
    let Part::Thinking(thinking) = &feed.reply().steps[0][0] else { panic!() };
    assert_eq!(thinking.label(feed.at(9.0)), "Thought for 1s");
}

#[test]
fn a_tool_line_spins_then_shows_its_result_or_error() {
    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(0.1, delta(DeltaKind::ToolCall, 0, "{\"cards\": [\"Deca"));
    // While its arguments stream it has no name yet, and shows the JSON so far.
    let Piece::Tool { call: streaming, .. } = &feed.reply().pieces()[0] else { panic!() };
    assert_eq!((streaming.name.as_str(), streaming.status(), streaming.arguments_text().as_str()), ("", ToolStatus::Running, "{\"cards\": [\"Deca"));
    feed.event(0.2, end(assistant(json!([call("t1", "synergy", json!({"cards": ["Decanter"]})), call("t2", "find_card", json!({"query": "x"}))]))));
    assert_eq!(tool(feed.reply(), "synergy").status(), ToolStatus::Running);
    assert_eq!(tool(feed.reply(), "synergy").arguments_text(), "{\n  \"cards\": [\n    \"Decanter\"\n  ]\n}");
    feed.event(0.3, end(result("t1", "synergy", json!({"n": 3}), false)));
    feed.event(0.4, end(json!({"role": "toolResult", "toolCallId": "t2", "toolName": "find_card", "content": [{"type": "text", "text": "Unknown card 'x': use find_card."}], "isError": true})));
    let reply = feed.reply();
    assert_eq!(tool(reply, "synergy").status(), ToolStatus::Done);
    assert_eq!(tool(reply, "synergy").result_text().unwrap(), "{\n  \"n\": 3\n}");
    assert_eq!(tool(reply, "find_card").status(), ToolStatus::Failed);
    // Without details, the text the model read.
    assert_eq!(tool(reply, "find_card").result_text().unwrap(), "Unknown card 'x': use find_card.");
}

#[test]
fn a_failed_suggest_follow_ups_or_ask_user_is_a_tool_line() {
    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(0.1, end(assistant(json!([
        call("s", "suggest", json!({"title": "T", "ranked": []})),
        call("f", "follow_ups", json!({"questions": ["one"]})),
        call("a", "ask_user", json!({"question": "Q?", "options": [{"id": "x", "label": "X"}]})),
    ]))));
    for (id, name) in [("s", "suggest"), ("f", "follow_ups"), ("a", "ask_user")] {
        feed.event(0.2, end(json!({"role": "toolResult", "toolCallId": id, "toolName": name, "content": [{"type": "text", "text": "bad"}], "isError": true})));
    }
    assert_eq!(kinds(feed.reply()), ["tool suggest", "tool follow_ups", "tool ask_user"]);
    assert!(feed.reply().follow_ups().is_empty());
}

#[test]
fn suggest_shows_a_pending_card_until_its_result_comes() {
    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(0.1, end(assistant(json!([call("s", "suggest", json!({"title": "Shop", "ranked": [{"card": "Decanter", "reason": "Fits"}]}))]))));
    let Piece::Suggest { card: pending, .. } = &feed.reply().pieces()[0] else { panic!() };
    assert!(pending.pending);
    assert_eq!((pending.title.as_str(), pending.rows.len(), pending.rows[0].card.as_str(), pending.rows[0].reason.as_str()), ("Shop", 1, "Decanter", "Fits"));
    assert_eq!((&pending.rows[0].data, &pending.source), (&RowData::Waiting, &None));
    // follow_ups is hidden while it runs, then buttons.
    feed.event(0.2, end(assistant(json!([call("f", "follow_ups", json!({"questions": ["A?", "B?"]}))]))));
    assert!(feed.reply().follow_ups().is_empty());
    assert_eq!(kinds(feed.reply()), ["suggest"]);
}

#[test]
fn stop_keeps_what_arrived_and_marks_the_reply_stopped() {
    let mut feed = Feed::new();
    feed.event(0.0, start(user("Which one?")));
    feed.event(0.0, end(user("Which one?")));
    feed.event(0.1, start(json!({"role": "assistant", "content": []})));
    feed.event(0.2, delta(DeltaKind::Thinking, 0, "hm"));
    feed.event(0.5, delta(DeltaKind::Text, 1, "Take the"));
    let at = feed.at(2.0);
    feed.transcript.stop(at);
    let reply = feed.reply();
    assert_eq!(reply.end, End::Stopped);
    assert_eq!(kinds(reply), ["thinking", "text Take the"]);
    assert!(!feed.transcript.streaming());
}

#[test]
fn stop_interrupts_a_running_tool_and_ends_thinking() {
    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(0.0, end(assistant(json!([call("t1", "rate_options", json!({}))]))));
    feed.event(0.1, start(json!({"role": "assistant", "content": []})));
    feed.event(0.1, delta(DeltaKind::Thinking, 0, "more"));
    let at = feed.at(3.0);
    feed.transcript.stop(at);
    let reply = feed.reply();
    assert_eq!(tool(reply, "rate_options").status(), ToolStatus::Failed);
    assert_eq!(tool(reply, "rate_options").result_text().unwrap(), INTERRUPTED);
    let Part::Thinking(thinking) = &reply.steps[1][0] else { panic!() };
    assert_eq!(thinking.label(feed.at(99.0)), "Thought for 3s");
}

#[test]
fn stop_before_anything_came_shows_a_stopped_reply() {
    let mut feed = Feed::new();
    feed.event(0.0, start(user("Hi")));
    feed.event(0.0, end(user("Hi")));
    let at = feed.at(1.0);
    feed.transcript.stop(at);
    assert_eq!(feed.reply().end, End::Stopped);
    assert!(feed.reply().steps.is_empty());
}

#[test]
fn a_failed_model_call_shows_its_error() {
    let mut feed = Feed::new();
    feed.event(0.0, start(user("Hi")));
    feed.event(0.1, start(json!({"role": "assistant", "content": []})));
    feed.event(0.2, end(json!({"role": "assistant", "content": [], "stopReason": "error", "errorMessage": "rate limited"})));
    feed.event(0.3, AdvisorEvent::Done);
    assert_eq!(feed.reply().end, End::Failed("rate limited".into()));
    assert!(feed.reply().steps.is_empty());
}

#[test]
fn an_error_event_or_a_broken_stream_fails_the_reply() {
    let mut feed = Feed::new();
    feed.event(0.0, start(user("Hi")));
    feed.event(0.1, AdvisorEvent::Error("the reply failed".into()));
    feed.event(0.2, AdvisorEvent::Done);
    assert_eq!(feed.reply().end, End::Failed("the reply failed".into()));

    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(0.1, delta(DeltaKind::Text, 0, "Ta"));
    let at = feed.at(1.0);
    feed.transcript.fail("the reply stopped: reset".into(), at);
    assert_eq!(feed.reply().end, End::Failed("the reply stopped: reset".into()));
    assert_eq!(kinds(feed.reply()), ["text Ta"]);
}

#[test]
fn a_stream_that_ends_without_done_is_done() {
    let mut feed = Feed::new();
    feed.event(0.0, start(json!({"role": "assistant", "content": []})));
    feed.event(0.1, end(assistant(json!([{"type": "text", "text": "ok"}]))));
    assert!(feed.transcript.streaming());
    let at = feed.at(1.0);
    feed.transcript.finish(at);
    assert_eq!(feed.reply().end, End::Done);
}

fn saved(seq: i64, message: Value, stats: Option<Value>) -> SavedMessage {
    SavedMessage { seq, message, stats }
}

fn ask_call() -> Value {
    call("q1", "ask_user", json!({"question": "Which plan?", "options": [{"id": "a", "label": "Weapons"}, {"id": "b", "label": "Shields"}], "multiple": true}))
}

#[test]
fn saved_messages_rebuild_the_replies_as_they_were() {
    let stats = json!({"model": "m", "elapsedMs": 900, "tokens": {"total": 3}, "costUsd": 0.01, "toolCalls": 1});
    let transcript = Transcript::from_saved(&[
        saved(0, user("Which one?"), None),
        saved(1, assistant(json!([{"type": "thinking", "thinking": "hm"}, call("t1", "rate_options", json!({}))])), None),
        saved(2, result("t1", "rate_options", json!({"n": 15}), false), None),
        saved(3, assistant(json!([{"type": "text", "text": "Take it."}])), Some(stats)),
        saved(4, user("And now?"), None),
        saved(5, json!({"role": "assistant", "content": [{"type": "text", "text": "Half"}], "stopReason": "aborted"}), None),
        // A reply stopped before anything came saved nothing: two questions in a row.
        saved(6, user("Again?"), None),
        saved(7, user("Hello?"), None),
        saved(8, json!({"role": "assistant", "content": [], "stopReason": "error", "errorMessage": "no key"}), None),
    ]);
    let turns: Vec<String> = transcript
        .turns
        .iter()
        .map(|turn| match turn {
            Turn::User(text) => format!("user {text}"),
            Turn::Reply(reply) => format!("reply {:?} {}", reply.end, kinds(reply).join(", ")),
        })
        .collect();
    assert_eq!(
        turns,
        [
            "user Which one?",
            "reply Done thinking, tool rate_options, text Take it.",
            "user And now?",
            "reply Stopped text Half",
            "user Again?",
            "user Hello?",
            "reply Failed(\"no key\") ",
        ]
    );
    let Turn::Reply(first) = &transcript.turns[1] else { panic!() };
    assert_eq!(first.stats.as_ref().unwrap().line(), ["m", "900 ms", "3 tokens", "$0.01", "1 tool call"]);
    assert_eq!(tool(first, "rate_options").status(), ToolStatus::Done);
    // A saved thinking's time isn't known.
    let Part::Thinking(thinking) = &first.steps[0][0] else { panic!() };
    assert_eq!(thinking.label(Instant::now()), "Thought");
}

#[test]
fn an_ask_user_card_waits_then_is_rebuilt_from_the_next_message() {
    let waiting = Transcript::from_saved(&[
        saved(0, user("Help"), None),
        saved(1, assistant(json!([{"type": "text", "text": "First:"}, ask_call()])), None),
        saved(2, result("q1", "ask_user", json!({"status": "waiting_for_user"}), false), None),
    ]);
    let Turn::Reply(reply) = &waiting.turns[1] else { panic!() };
    assert_eq!(kinds(reply), ["text First:", "ask_user"]);
    let (call_id, ask) = waiting.waiting_question().unwrap();
    assert_eq!(call_id, "q1");
    assert_eq!(ask.options, [Choice { id: "a".into(), label: "Weapons".into() }, Choice { id: "b".into(), label: "Shields".into() }]);
    assert_eq!(waiting.answer_after(1), None);

    let answered = Transcript::from_saved(&[
        saved(0, user("Help"), None),
        saved(1, assistant(json!([ask_call()])), None),
        saved(2, result("q1", "ask_user", json!({"status": "waiting_for_user"}), false), None),
        saved(3, user("- Shields\nand gold"), None),
        saved(4, assistant(json!([{"type": "text", "text": "Shields it is."}])), None),
    ]);
    assert_eq!(answered.waiting_question(), None);
    let answer = answered.answer_after(1).unwrap();
    assert_eq!(answer, "- Shields\nand gold");
    let read = ask.read(answer);
    assert_eq!(read.picked.into_iter().collect::<Vec<_>>(), ["b"]);
    assert_eq!(read.rest, "and gold");
}

#[test]
fn saved_step_stats_are_passed_over_and_the_usage_line_is_the_replys_total() {
    // The API gives every assistant message its `step`, and the reply's total as `stats` on the last only.
    let step = json!({"model": "a", "elapsedMs": 300, "tokens": {"total": 1}, "costUsd": 0.001, "toolCalls": 1});
    let saved: Saved = serde_json::from_value(json!({"running": false, "messages": [
        {"seq": 0, "message": user("Which one?"), "stats": null, "step": null, "createdMs": 1},
        {"seq": 1, "message": assistant(json!([call("t1", "rate_options", json!({}))])), "stats": null, "step": step, "createdMs": 2},
        {"seq": 2, "message": result("t1", "rate_options", json!({"n": 15}), false), "stats": null, "step": null, "createdMs": 3},
        {"seq": 3, "message": assistant(json!([{"type": "text", "text": "Take it."}])), "step": step,
         "stats": {"model": "m", "elapsedMs": 900, "tokens": {"total": 3}, "costUsd": 0.01, "toolCalls": 1}, "createdMs": 4},
    ]}))
    .unwrap();
    let transcript = Transcript::from_saved(&saved.messages);
    let Turn::Reply(reply) = &transcript.turns[1] else { panic!() };
    assert_eq!(reply.stats.as_ref().unwrap().line(), ["m", "900 ms", "3 tokens", "$0.01", "1 tool call"]);
}
