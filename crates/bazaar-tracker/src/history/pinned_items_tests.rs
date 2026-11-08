use crate::history::History;
use crate::run::{Event, EventKind};

fn open() -> (tempfile::TempDir, History) {
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.db")).unwrap();
    (dir, history)
}

#[test]
fn items_are_pinned_by_hero_in_the_order_pinned() {
    let (_dir, history) = open();
    history.pin_item("Jules", "TPL-B", 20).unwrap();
    history.pin_item("Jules", "tpl-a", 10).unwrap();
    history.pin_item("Jules", "tpl-a", 30).unwrap();
    assert_eq!(history.pinned_items("Jules").unwrap(), ["tpl-a", "tpl-b"]);
    assert!(history.pinned_items("Vanessa").unwrap().is_empty());
}

#[test]
fn unpinning_removes_only_that_item_for_that_hero() {
    let (_dir, history) = open();
    history.pin_item("Jules", "tpl-a", 10).unwrap();
    history.pin_item("Jules", "tpl-b", 20).unwrap();
    history.pin_item("Vanessa", "tpl-a", 30).unwrap();
    history.unpin_item("Jules", "TPL-A").unwrap();
    assert_eq!(history.pinned_items("Jules").unwrap(), ["tpl-b"]);
    assert_eq!(history.pinned_items("Vanessa").unwrap(), ["tpl-a"]);
}

#[test]
fn pinned_items_stay_when_a_run_is_deleted() {
    let (_dir, history) = open();
    let kind = EventKind::RunStarted { play_mode: None, build_id: None, environment: None, hero: None, rating: None, rank: None, rank_division: None };
    history.insert_event(&Event { id: "e1".into(), run_id: "r1".into(), timestamp_ms: 1, day: None, hour: None, kind }).unwrap();
    history.pin_item("Jules", "tpl-a", 10).unwrap();
    history.delete_run("r1").unwrap();
    assert_eq!(history.pinned_items("Jules").unwrap(), ["tpl-a"]);
}
