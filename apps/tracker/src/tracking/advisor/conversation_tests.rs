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

