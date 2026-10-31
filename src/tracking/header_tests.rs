use super::*;
use crate::tracking::test_events::*;

fn run() -> Run {
    Run { id: "r1".into(), started_ms: 1_793_394_840_000, hero: Some("Vanessa".into()), day: Some(4), victories: Some(3), losses: Some(1), ..Run::default() }
}

fn started(rating: Option<i32>, rank: Option<&str>, division: Option<i32>) -> Event {
    event("e0", 0, EventKind::RunStarted { play_mode: None, build_id: None, environment: None, hero: None, rating, rank: rank.map(str::to_owned), rank_division: division })
}

fn ended(rating_after: Option<i32>) -> Event {
    event(
        "end",
        900,
        EventKind::RunEnded {
            outcome: Outcome::Win,
            victories: None,
            losses: None,
            gold: None,
            health_max: None,
            prestige: None,
            level: None,
            income: None,
            rating_after,
            rank_after: None,
            rank_division_after: None,
        },
    )
}

fn on_day(day: i32, mut event: Event) -> Event {
    event.day = Some(day);
    event
}

#[test]
fn the_header_has_the_runs_facts() {
    let mut run = run();
    run.outcome = Some(Outcome::Win);
    let header = header(&run, &[started(Some(812), Some("Gold"), Some(3))], true);
    assert_eq!(header.hero.as_deref(), Some("Vanessa"));
    assert_eq!(header.rank.as_deref(), Some("Gold 3 · 812"));
    assert_eq!((header.day.as_deref(), header.record.as_str()), (Some("Day 4"), "3W–1L"));
    assert_eq!((header.live, header.outcome), (true, Some(Outcome::Win)));
    assert!(header.started.contains("/2026 "));
}

#[test]
fn an_unknown_outcome_is_no_outcome() {
    let mut run = run();
    run.outcome = Some(Outcome::Unknown);
    assert_eq!(header(&run, &[], false).outcome, None);
}

#[test]
fn the_standing_is_whatever_the_run_started_event_knows() {
    assert_eq!(header(&run(), &[started(Some(900), None, None)], false).rank.as_deref(), Some("900"));
    assert_eq!(header(&run(), &[started(None, Some("Silver"), None)], false).rank.as_deref(), Some("Silver"));
    assert_eq!(header(&run(), &[started(None, None, None)], false).rank, None);
    assert_eq!(header(&run(), &[], false).rank, None);
}

#[test]
fn the_rating_change_needs_a_rating_before_and_after() {
    assert_eq!(header(&run(), &[started(Some(812), None, None), ended(Some(836))], false).rating_change.as_deref(), Some("+24"));
    assert_eq!(header(&run(), &[started(Some(812), None, None), ended(Some(800))], false).rating_change.as_deref(), Some("-12"));
    assert_eq!(header(&run(), &[started(Some(812), None, None)], false).rating_change, None);
    assert_eq!(header(&run(), &[started(None, None, None), ended(Some(836))], false).rating_change, None);
}

#[test]
fn the_timeline_marks_each_day_by_its_last_combat() {
    let events = [
        on_day(1, combat_end("c1", 1, Outcome::Win)),
        on_day(2, combat_end("c2", 2, Outcome::Loss)),
        on_day(2, combat_end("c3", 3, Outcome::Win)),
        on_day(4, combat_end("c4", 4, Outcome::Unknown)),
    ];
    let timeline = header(&run(), &events, true).timeline;
    let marks: Vec<_> = timeline.iter().map(|node| (node.day, node.result)).collect();
    assert_eq!(marks, [(1, Some(Outcome::Win)), (2, Some(Outcome::Win)), (3, None), (4, None)]);
}

#[test]
fn the_timeline_reaches_the_latest_day_of_the_run_or_its_events() {
    let mut run = run();
    run.day = Some(2);
    assert_eq!(header(&run, &[on_day(5, combat_end("c", 1, Outcome::Win))], true).timeline.len(), 5);
    run.day = None;
    assert!(header(&run, &[], true).timeline.is_empty());
}
