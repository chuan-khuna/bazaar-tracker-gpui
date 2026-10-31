use bazaar_tracker::memory::Snapshot;
use bazaar_tracker::run::PresentedOption;

use super::*;
use crate::tracking::test_events::*;

fn started() -> Event {
    event("e0", 1_000, EventKind::RunStarted { play_mode: Some("Ranked".into()), build_id: None, environment: None, hero: Some("Vanessa".into()), rating: None, rank: None, rank_division: None })
}

fn at(mut event: Event, day: Option<i32>, hour: Option<i32>) -> Event {
    event.day = day;
    event.hour = hour;
    event
}

fn rows_of(events: &[Event]) -> Vec<EventRow> {
    rows(events, None, Filter::default(), &Names::default())
}

fn selected(id: &str, template: &str, tier: Option<&str>) -> Event {
    event(
        id,
        0,
        EventKind::Selected {
            card_kind: "Item".into(),
            instance_id: "itm_1".into(),
            template_id: Some(template.into()),
            tier: tier.map(str::to_owned),
            enchantment: None,
            section: None,
            target: None,
            merged_into: None,
            options_event_id: None,
            snapshot: Snapshot::default(),
        },
    )
}

#[test]
fn a_run_started_row_names_the_hero_and_mode() {
    let row = &rows_of(&[started()])[0];
    assert_eq!((row.kind, row.description.as_str(), row.time.as_str()), ("Run Started", "Vanessa · Ranked", "0:00"));
}

#[test]
fn options_say_how_many_and_in_which_state() {
    let option = PresentedOption { instance_id: "i".into(), ..PresentedOption::default() };
    let three = event("o", 0, EventKind::OptionsPresented { context: "ChoiceState".into(), options: vec![option.clone(); 3], snapshot: Snapshot::default() });
    let one = event("o", 0, EventKind::OptionsPresented { context: "LootState".into(), options: vec![option], snapshot: Snapshot::default() });
    assert_eq!(rows_of(&[three])[0].description, "3 options · Choice");
    assert_eq!(rows_of(&[one])[0].description, "1 option · Loot");
}

#[test]
fn a_selection_is_a_sub_row_with_the_name_tier_and_kind() {
    let row = &rows_of(&[selected("s", "tpl-x", Some("Bronze"))])[0];
    assert_eq!((row.kind, row.sub, row.description.as_str()), ("Selected", true, "tpl-x (Bronze): Item"));
    assert_eq!(rows_of(&[selected("s", "tpl-x", None)])[0].description, "tpl-x: Item");
}

#[test]
fn a_sale_is_a_sub_row() {
    let row = &rows_of(&[sold("s", 0, Snapshot::default())])[0];
    assert!((row.kind, row.sub) == ("Sold", true) && row.description == "itm_x");
}

#[test]
fn other_rows_are_not_sub_rows() {
    assert!(rows_of(&[started(), event("r", 0, EventKind::Rerolled)]).iter().all(|row| !row.sub));
}

#[test]
fn a_combat_start_says_pvp_or_pve() {
    let pvp = event("c", 0, EventKind::CombatStart { player: Snapshot::default(), opponent: Snapshot::default(), is_pvp: true, opponent_name: Some("Bob".into()), opponent_hero: Some("Mak".into()), opponent_rating: None, opponent_level: None });
    let pve = event("c", 0, EventKind::CombatStart { player: Snapshot::default(), opponent: Snapshot::default(), is_pvp: false, opponent_name: None, opponent_hero: None, opponent_rating: None, opponent_level: None });
    assert_eq!(rows_of(&[pvp])[0].description, "PvP vs Bob (Mak)");
    assert_eq!(rows_of(&[pve])[0].description, "PvE");
    assert_eq!(rows_of(&[combat_start("c", 0, Snapshot::default(), Snapshot::default())])[0].description, "PvP");
}

#[test]
fn a_combat_end_is_a_victory_or_a_defeat_in_its_colour() {
    let rows = rows_of(&[combat_end("a", 0, Outcome::Win), combat_end("b", 0, Outcome::Loss), combat_end("c", 0, Outcome::Unknown)]);
    let got: Vec<_> = rows.iter().map(|row| (row.description.as_str(), row.tone)).collect();
    assert_eq!(got, [("Victory", Tone::Win), ("Defeat", Tone::Loss), ("Ended", Tone::Plain)]);
}

#[test]
fn a_run_end_adds_the_record() {
    let end = event(
        "e",
        0,
        EventKind::RunEnded {
            outcome: Outcome::Win,
            victories: Some(10),
            losses: Some(2),
            gold: None,
            health_max: None,
            prestige: None,
            level: None,
            income: None,
            rating_after: None,
            rank_after: None,
            rank_division_after: None,
        },
    );
    let row = &rows_of(&[end])[0];
    assert_eq!((row.description.as_str(), row.tone), ("Victory · 10W–2L", Tone::Win));
}

#[test]
fn time_is_counted_from_the_first_event() {
    let rows = rows_of(&[started(), event("a", 66_000, EventKind::Rerolled)]);
    assert_eq!(rows[1].time, "1:05");
}

#[test]
fn every_row_has_its_day_and_hour_and_says_when_they_change() {
    let events = [
        at(started(), Some(1), Some(1)),
        at(event("a", 0, EventKind::Rerolled), Some(1), Some(2)),
        at(event("b", 0, EventKind::Rerolled), Some(1), Some(2)),
        at(event("c", 0, EventKind::Rerolled), Some(2), Some(1)),
        at(event("d", 0, EventKind::Rerolled), None, None),
        at(event("e", 0, EventKind::Rerolled), Some(2), Some(1)),
    ];
    let labels: Vec<_> = rows_of(&events).into_iter().map(|row| (row.day, row.hour, row.moved)).collect();
    let expected = [("D1", "H1", true), ("D1", "H2", true), ("D1", "H2", false), ("D2", "H1", true), ("D2", "H1", false), ("D2", "H1", false)];
    assert_eq!(labels, expected.map(|(day, hour, moved)| (day.to_owned(), hour.to_owned(), moved)));
}

#[test]
fn the_selected_event_is_marked() {
    let events = [started(), event("a", 0, EventKind::Rerolled)];
    let rows = rows(&events, Some("a"), Filter::default(), &Names::default());
    assert_eq!(rows.iter().map(|row| row.selected).collect::<Vec<_>>(), [false, true]);
}

/// Day 1 hours 0 and 1, then day 2 hour 1, with an Event in each that has no day or hour of its own.
fn a_day_and_a_bit() -> Vec<Event> {
    vec![
        at(started(), Some(1), Some(0)),
        at(event("a", 2_000, EventKind::Rerolled), None, None),
        at(event("b", 3_000, EventKind::Rerolled), Some(1), Some(1)),
        at(event("c", 4_000, EventKind::Rerolled), Some(2), Some(1)),
        at(event("d", 5_000, EventKind::Rerolled), None, None),
    ]
}

fn ids(events: &[Event], filter: Filter) -> Vec<String> {
    rows(events, None, filter, &Names::default()).into_iter().map(|row| row.id).collect()
}

#[test]
fn the_filter_keeps_a_day_an_hour_or_both_counting_events_without_their_own_in_the_last_one() {
    let events = a_day_and_a_bit();
    assert_eq!(ids(&events, Filter::default()), ["e0", "a", "b", "c", "d"]);
    assert_eq!(ids(&events, Filter { day: Some(1), hour: None }), ["e0", "a", "b"]);
    assert_eq!(ids(&events, Filter { day: None, hour: Some(1) }), ["b", "c", "d"]);
    assert_eq!(ids(&events, Filter { day: Some(2), hour: Some(1) }), ["c", "d"]);
    assert!(ids(&events, Filter { day: Some(3), hour: None }).is_empty());
}

#[test]
fn a_filtered_list_keeps_the_runs_times_and_marks_its_first_row() {
    let rows = rows(&a_day_and_a_bit(), None, Filter { day: Some(2), hour: None }, &Names::default());
    assert_eq!((rows[0].time.as_str(), rows[0].day.as_str(), rows[0].hour.as_str(), rows[0].moved), ("0:03", "D2", "H1", true));
}

#[test]
fn the_filter_offers_every_day_up_to_the_latest_and_hour_0_only_when_used() {
    let events = a_day_and_a_bit();
    assert_eq!(days(&events), [1, 2]);
    assert_eq!(hours(&events), [0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(hours(&events[1..]), [1, 2, 3, 4, 5, 6]);
    assert!(days(&[]).is_empty());
}
