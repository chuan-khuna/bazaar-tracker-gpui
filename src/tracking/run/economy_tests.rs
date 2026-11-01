use super::*;
use crate::tracking::test_events::*;

use bazaar_tracker::run::EventKind;

fn at(mut event: Event, day: Option<i32>, hour: Option<i32>) -> Event {
    event.day = day;
    event.hour = hour;
    event
}

fn golds(events: &[Event]) -> Vec<(i32, i32, i32)> {
    by_hour(events).into_iter().map(|point| (point.day, point.hour, point.gold)).collect()
}

#[test]
fn each_hour_takes_the_gold_of_its_first_snapshot() {
    let events = [
        at(options("a", 0, snapshot(10, &[])), Some(1), Some(1)),
        at(sold("b", 1, snapshot(14, &[])), Some(1), Some(1)),
        at(options("c", 2, snapshot(20, &[])), Some(1), Some(2)),
        at(options("d", 3, snapshot(7, &[])), Some(2), Some(1)),
    ];
    assert_eq!(golds(&events), [(1, 1, 10), (1, 2, 20), (2, 1, 7)]);
}

#[test]
fn an_hour_with_no_snapshot_at_its_start_takes_the_first_one_in_it() {
    let events = [at(event("r", 0, EventKind::Rerolled), Some(1), Some(1)), at(event("x", 1, EventKind::Rerolled), None, None), at(sold("s", 2, snapshot(9, &[])), None, None)];
    assert_eq!(golds(&events), [(1, 1, 9)]);
}

#[test]
fn hours_without_gold_or_before_the_run_has_an_hour_are_left_out() {
    let mut no_gold = snapshot(0, &[]);
    no_gold.stats.gold = None;
    let events = [options("early", 0, snapshot(5, &[])), at(options("a", 1, no_gold), Some(1), Some(1)), at(event("r", 2, EventKind::Rerolled), Some(1), Some(2))];
    assert!(by_hour(&events).is_empty());
}

#[test]
fn days_span_their_hours_in_order() {
    let point = |day, hour| HourGold { day, hour, gold: 0, income: None };
    let points = [point(1, 1), point(1, 2), point(1, 4), point(2, 1), point(3, 1), point(3, 2)];
    assert_eq!(days(&points), [DaySpan { day: 1, first: 0, last: 2 }, DaySpan { day: 2, first: 3, last: 3 }, DaySpan { day: 3, first: 4, last: 5 }]);
    assert_eq!(days(&[]), []);
}
