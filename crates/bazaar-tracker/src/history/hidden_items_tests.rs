use crate::history::History;
use crate::run::{Event, EventKind};

fn open() -> (tempfile::TempDir, History) {
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.db")).unwrap();
    (dir, history)
}

fn start(history: &History, event_id: &str, run_id: &str) {
    let kind = EventKind::RunStarted { play_mode: None, build_id: None, environment: None, hero: None, rating: None, rank: None, rank_division: None };
    history.insert_event(&Event { id: event_id.into(), run_id: run_id.into(), timestamp_ms: 1, day: None, hour: None, kind }).unwrap();
}

#[test]
fn items_are_hidden_by_run_and_shown_again() {
    let (_dir, history) = open();
    start(&history, "e1", "r1");
    start(&history, "e2", "r2");
    history.hide_item("r1", "TPL-B", 20).unwrap();
    history.hide_item("r1", "tpl-a", 10).unwrap();
    history.hide_item("r1", "tpl-a", 30).unwrap();
    assert_eq!(history.hidden_items("r1").unwrap(), ["tpl-a", "tpl-b"]);
    assert!(history.hidden_items("r2").unwrap().is_empty());
    history.unhide_item("r1", "TPL-A").unwrap();
    assert_eq!(history.hidden_items("r1").unwrap(), ["tpl-b"]);
}

#[test]
fn hidden_items_follow_a_rebound_run_and_go_with_a_deleted_one() {
    let (_dir, history) = open();
    start(&history, "e1", "local-1");
    history.hide_item("local-1", "tpl-a", 10).unwrap();
    history.rebind_run("local-1", "real").unwrap();
    assert_eq!(history.hidden_items("real").unwrap(), ["tpl-a"]);

    start(&history, "e2", "local-2");
    history.hide_item("local-2", "tpl-b", 20).unwrap();
    history.rebind_run("local-2", "real").unwrap();
    assert_eq!(history.hidden_items("real").unwrap(), ["tpl-a", "tpl-b"], "merged into the existing run");

    history.delete_run("real").unwrap();
    assert!(history.hidden_items("real").unwrap().is_empty());
}
