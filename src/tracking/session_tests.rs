use super::*;

fn open() -> (tempfile::TempDir, History) {
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join(history::FILE_NAME)).unwrap();
    (dir, history)
}

fn event(id: &str, run: &str, at: i64, kind: EventKind) -> Event {
    Event { id: id.into(), run_id: run.into(), timestamp_ms: at, day: None, hour: None, kind }
}

fn started(id: &str, run: &str, at: i64, hero: &str) -> Event {
    event(id, run, at, EventKind::RunStarted { play_mode: None, build_id: None, environment: None, hero: Some(hero.into()), rating: None, rank: None, rank_division: None })
}

fn ended(id: &str, run: &str, at: i64, outcome: Outcome) -> Event {
    event(id, run, at, EventKind::RunEnded { outcome, victories: Some(7), losses: Some(2), gold: None, health_max: None, prestige: None, level: None, income: None, rating_after: None, rank_after: None, rank_division_after: None })
}

/// Save an event to the history and tell the session about it, as the tracker does.
fn save(session: &mut Session, history: &History, event: Event) {
    history.insert_event(&event).unwrap();
    session.apply(&Notice::EventSaved(event), history).unwrap();
}

fn ids(session: &Session) -> Vec<&str> {
    session.runs().iter().map(|run| run.id.as_str()).collect()
}

#[test]
fn the_list_is_read_from_the_history_newest_first() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "old", 100, "Mak")).unwrap();
    history.insert_event(&started("e2", "new", 300, "Jules")).unwrap();
    history.insert_event(&started("e3", "middle", 200, "Vanessa")).unwrap();
    let session = Session::load(&history).unwrap();
    assert_eq!(ids(&session), ["new", "middle", "old"]);
    assert_eq!(session.live(), None);
    assert_eq!(session.selected(), None);
}

#[test]
fn a_new_run_goes_on_top_and_is_live_and_selected() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "old", 100, "Mak")).unwrap();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e2", "new", 500, "Jules"));
    assert_eq!(ids(&session), ["new", "old"]);
    assert_eq!(session.live(), Some("new"));
    assert_eq!(session.selected(), Some("new"));
}

#[test]
fn a_changed_run_is_updated_in_place() {
    let (_dir, history) = open();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e1", "a", 100, "Mak"));
    save(&mut session, &history, started("e2", "b", 200, "Jules"));
    let mut won = ended("e3", "a", 300, Outcome::Win);
    won.day = Some(9);
    save(&mut session, &history, won);
    assert_eq!(ids(&session), ["b", "a"], "a Run keeps its place by its start, not by its latest Event");
    let run = &session.runs()[1];
    assert_eq!((run.victories, run.losses, run.day), (Some(7), Some(2), Some(9)));
    assert!(run.ended);
}

#[test]
fn the_live_run_ends_with_its_run_ended_event() {
    let (_dir, history) = open();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e1", "a", 100, "Mak"));
    assert_eq!(session.live(), Some("a"));
    save(&mut session, &history, ended("e2", "a", 200, Outcome::Win));
    assert_eq!(session.live(), None);
    assert_eq!(session.selected(), Some("a"), "the selection stays");
}

#[test]
fn losing_the_game_ends_the_live_run() {
    let (_dir, history) = open();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e1", "a", 100, "Mak"));
    session.apply(&Notice::Connection(ConnectionChange::Disconnected { reason: "gone".into() }), &history).unwrap();
    assert_eq!(session.live(), None);
}

#[test]
fn a_run_that_goes_live_takes_the_selection_from_another() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "old", 100, "Mak")).unwrap();
    let mut session = Session::load(&history).unwrap();
    session.select("old", &history).unwrap();
    save(&mut session, &history, started("e2", "new", 500, "Jules"));
    assert_eq!(session.selected(), Some("new"));
    // Later Events of the same Run do not take it back from a Run the player chose.
    session.select("old", &history).unwrap();
    save(&mut session, &history, event("e3", "new", 600, EventKind::Rerolled));
    assert_eq!(session.selected(), Some("old"));
}

#[test]
fn only_a_known_run_can_be_selected() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "a", 100, "Mak")).unwrap();
    let mut session = Session::load(&history).unwrap();
    session.select("nope", &history).unwrap();
    assert_eq!(session.selected(), None);
    session.select("a", &history).unwrap();
}

#[test]
fn a_rebound_run_stays_one_row_and_keeps_being_live_and_selected() {
    let (_dir, history) = open();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e1", "local-1", 100, "Mak"));
    assert!(history.rebind_run("local-1", "real-1").unwrap());
    session.apply(&Notice::RunRebound { old_run_id: "local-1".into(), new_run_id: "real-1".into() }, &history).unwrap();
    assert_eq!(ids(&session), ["real-1"]);
    assert_eq!(session.live(), Some("real-1"));
    assert_eq!(session.selected(), Some("real-1"));
}

#[test]
fn a_run_merged_into_an_existing_one_leaves_one_row() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "real-1", 200, "Mak")).unwrap();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e2", "local-1", 100, "Mak"));
    history.rebind_run("local-1", "real-1").unwrap();
    session.apply(&Notice::RunRebound { old_run_id: "local-1".into(), new_run_id: "real-1".into() }, &history).unwrap();
    assert_eq!(ids(&session), ["real-1"]);
}

#[test]
fn rows_say_what_the_sidebar_shows() {
    let (_dir, history) = open();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e1", "a", 100, "Mak"));
    let mut won = ended("e2", "a", 200, Outcome::Win);
    won.day = Some(9);
    save(&mut session, &history, won);
    save(&mut session, &history, started("e3", "b", 300, "Jules"));
    let rows = session.rows();
    assert_eq!(rows.len(), 2);
    let live = &rows[0];
    assert_eq!((live.id.as_str(), live.hero.as_str(), live.record.as_str()), ("b", "Jules", "0W–0L"));
    assert!(live.live && live.selected);
    assert_eq!((live.outcome, live.day.as_deref()), (None, None));
    let done = &rows[1];
    assert_eq!((done.hero.as_str(), done.record.as_str()), ("Mak", "7W–2L"));
    assert!(!done.live && !done.selected);
    assert_eq!((done.outcome, done.day.as_deref()), (Some(Outcome::Win), Some("Day 9")));
}

#[test]
fn a_run_with_no_hero_is_still_listed() {
    let (_dir, history) = open();
    history.insert_event(&event("e1", "a", 100, EventKind::Rerolled)).unwrap();
    let session = Session::load(&history).unwrap();
    assert_eq!(session.rows()[0].hero, "Unknown hero");
}

#[test]
fn selecting_a_run_loads_its_events_in_order() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "a", 100, "Mak")).unwrap();
    history.insert_event(&event("e2", "a", 200, EventKind::Rerolled)).unwrap();
    history.insert_event(&started("e3", "b", 300, "Jules")).unwrap();
    let mut session = Session::load(&history).unwrap();
    assert!(session.events().is_empty());
    session.select("a", &history).unwrap();
    let ids: Vec<_> = session.events().iter().map(|event| event.id.as_str()).collect();
    assert_eq!(ids, ["e1", "e2"]);
    assert_eq!(session.selected_run().map(|run| run.id.as_str()), Some("a"));
    session.select("b", &history).unwrap();
    assert_eq!(session.events().len(), 1, "another Run's Events replace them");
}

#[test]
fn saved_events_of_the_selected_run_are_added_and_others_are_not() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "old", 100, "Mak")).unwrap();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e2", "new", 500, "Jules"));
    assert_eq!(session.events().len(), 1, "the Run that went live is selected with its first Event");
    save(&mut session, &history, event("e3", "new", 600, EventKind::Rerolled));
    assert_eq!(session.events().len(), 2);
    // An Event of a Run that is not selected leaves the Events alone.
    session.select("old", &history).unwrap();
    save(&mut session, &history, event("e4", "new", 700, EventKind::Rerolled));
    assert_eq!(session.events().len(), 1);
}

#[test]
fn an_event_seen_twice_is_listed_once() {
    let (_dir, history) = open();
    let mut session = Session::load(&history).unwrap();
    let first = started("e1", "a", 100, "Mak");
    save(&mut session, &history, first.clone());
    session.apply(&Notice::EventSaved(first), &history).unwrap();
    assert_eq!(session.events().len(), 1);
}

#[test]
fn a_rebound_selected_run_reloads_its_events() {
    let (_dir, history) = open();
    let mut session = Session::load(&history).unwrap();
    save(&mut session, &history, started("e1", "local-1", 100, "Mak"));
    save(&mut session, &history, event("e2", "local-1", 200, EventKind::Rerolled));
    history.rebind_run("local-1", "real-1").unwrap();
    session.apply(&Notice::RunRebound { old_run_id: "local-1".into(), new_run_id: "real-1".into() }, &history).unwrap();
    assert_eq!(session.events().len(), 2);
    assert!(session.events().iter().all(|event| event.run_id == "real-1"));
}

#[test]
fn an_event_can_be_chosen_and_let_go_of() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "a", 100, "Mak")).unwrap();
    history.insert_event(&event("e2", "a", 200, EventKind::Rerolled)).unwrap();
    let mut session = Session::load(&history).unwrap();
    session.select("a", &history).unwrap();
    session.toggle_event("e2");
    assert_eq!(session.selected_event(), Some("e2"));
    session.toggle_event("e1");
    assert_eq!(session.selected_event(), Some("e1"), "choosing another replaces it");
    session.toggle_event("e1");
    assert_eq!(session.selected_event(), None);
    session.toggle_event("nope");
    assert_eq!(session.selected_event(), None, "only an Event of the Run can be chosen");
}

#[test]
fn choosing_another_run_clears_the_chosen_event_and_reselecting_the_same_run_keeps_it() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "a", 100, "Mak")).unwrap();
    history.insert_event(&started("e2", "b", 200, "Jules")).unwrap();
    let mut session = Session::load(&history).unwrap();
    session.select("a", &history).unwrap();
    session.toggle_event("e1");
    session.select("a", &history).unwrap();
    assert_eq!(session.selected_event(), Some("e1"));
    session.select("b", &history).unwrap();
    assert_eq!(session.selected_event(), None);
}

#[test]
fn a_run_going_live_clears_the_chosen_event() {
    let (_dir, history) = open();
    history.insert_event(&started("e1", "old", 100, "Mak")).unwrap();
    let mut session = Session::load(&history).unwrap();
    session.select("old", &history).unwrap();
    session.toggle_event("e1");
    save(&mut session, &history, started("e2", "new", 500, "Jules"));
    assert_eq!(session.selected_event(), None);
}
