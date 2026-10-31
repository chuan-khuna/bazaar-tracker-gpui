use bazaar_tracker::run::Outcome;

use super::*;
use crate::tracking::test_events::*;

fn run() -> Vec<Event> {
    vec![
        event("started", 0, EventKind::RunStarted { play_mode: None, build_id: None, environment: None, hero: None, rating: None, rank: None, rank_division: None }),
        options("options", 10, snapshot(10, &[])),
        sold("sold", 20, snapshot(20, &[])),
        combat_start("fight", 30, snapshot(30, &[]), snapshot(99, &[])),
        combat_end("fight-over", 40, Outcome::Win),
        event("reroll", 50, EventKind::Rerolled),
    ]
}

#[test]
fn with_nothing_selected_it_is_the_latest_snapshot() {
    assert_eq!(gold_of(at(&run(), None)), Some(30));
}

#[test]
fn a_combat_start_contributes_the_players_side() {
    let events = run();
    assert_eq!(gold_of(at(&events, Some("fight"))), Some(30));
}

#[test]
fn a_selected_event_with_a_snapshot_shows_its_own() {
    let events = run();
    assert_eq!(gold_of(at(&events, Some("options"))), Some(10));
    assert_eq!(gold_of(at(&events, Some("sold"))), Some(20));
}

#[test]
fn a_selected_event_without_one_shows_the_nearest_before_it() {
    let events = run();
    assert_eq!(gold_of(at(&events, Some("fight-over"))), Some(30));
    assert_eq!(gold_of(at(&events, Some("reroll"))), Some(30));
}

#[test]
fn an_event_before_any_snapshot_shows_none() {
    let events = run();
    assert!(at(&events, Some("started")).is_none());
}

#[test]
fn an_unknown_selection_counts_as_none() {
    assert_eq!(gold_of(at(&run(), Some("gone"))), Some(30));
}

#[test]
fn a_run_with_no_events_has_no_snapshot() {
    assert!(at(&[], None).is_none());
}

fn days() -> Vec<Event> {
    let day = |mut event: Event, day: i32| {
        event.day = Some(day);
        event
    };
    vec![
        day(options("d1a", 10, snapshot(11, &[])), 1),
        day(sold("d1b", 20, snapshot(12, &[])), 1),
        day(combat_end("d1c", 30, Outcome::Win), 1),
        day(event("d2r", 40, EventKind::Rerolled), 2),
        day(options("d3a", 50, snapshot(31, &[])), 3),
        day(sold("d3b", 60, snapshot(32, &[])), 3),
    ]
}

#[test]
fn a_day_shows_its_last_snapshot() {
    let events = days();
    assert_eq!(gold_of(at_day(&events, 1)), Some(12));
    assert_eq!(gold_of(at_day(&events, 3)), Some(32));
}

#[test]
fn a_day_with_no_snapshot_shows_the_nearest_before_it() {
    assert_eq!(gold_of(at_day(&days(), 2)), Some(12));
}

#[test]
fn a_day_after_the_run_shows_the_last_snapshot_and_one_before_it_shows_none() {
    assert_eq!(gold_of(at_day(&days(), 9)), Some(32));
    assert!(at_day(&days(), 0).is_none());
}

#[test]
fn events_without_a_day_are_passed_over_by_the_day_lookup() {
    let mut events = days();
    events.insert(2, sold("nodays", 25, snapshot(99, &[])));
    assert_eq!(gold_of(at_day(&events, 1)), Some(12));
}

#[test]
fn a_choice_picks_the_lookup() {
    let events = days();
    assert_eq!(gold_of(choose(&events, Choice::Latest)), Some(32));
    assert_eq!(gold_of(choose(&events, Choice::Event("d1a"))), Some(11));
    assert_eq!(gold_of(choose(&events, Choice::Day(1))), Some(12));
}
