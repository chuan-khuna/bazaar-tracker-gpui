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
fn a_combat_start_has_a_pvp_or_pve_badge_and_names_the_opponent() {
    let pvp = event("c", 0, EventKind::CombatStart { player: Snapshot::default(), opponent: Snapshot::default(), is_pvp: true, opponent_name: Some("Bob".into()), opponent_hero: Some("Mak".into()), opponent_rating: None, opponent_level: None });
    let pve = event("c", 0, EventKind::CombatStart { player: Snapshot::default(), opponent: Snapshot::default(), is_pvp: false, opponent_name: None, opponent_hero: None, opponent_rating: None, opponent_level: None });
    let pvp = &rows_of(&[pvp])[0];
    assert_eq!((pvp.badge, pvp.description.as_str()), (Some("PvP"), "vs Bob (Mak)"));
    let pve = &rows_of(&[pve])[0];
    assert_eq!((pve.badge, pve.description.as_str()), (Some("PvE"), ""));
    let unnamed = &rows_of(&[combat_start("c", 0, Snapshot::default(), Snapshot::default())])[0];
    assert_eq!((unnamed.badge, unnamed.description.as_str()), (Some("PvP"), ""));
    assert_eq!(rows_of(&[started()])[0].badge, None, "other Events have none");
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
fn the_first_row_of_an_hour_has_the_gold_at_its_start() {
    let events = [
        at(event("r", 0, EventKind::Rerolled), Some(1), Some(1)),
        at(options("a", 0, snapshot(10, &[])), Some(1), Some(1)),
        at(sold("b", 0, snapshot(14, &[])), Some(1), Some(1)),
        at(options("c", 0, snapshot(20, &[])), Some(1), Some(2)),
        at(event("x", 0, EventKind::Rerolled), Some(1), Some(3)),
    ];
    let golds: Vec<_> = rows_of(&events).into_iter().map(|row| row.gold).collect();
    assert_eq!(golds, [Some(10), None, None, Some(20), None], "an hour with no Snapshot has no gold");
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
    assert_eq!(ids(&events, Filter { day: Some(1), hour: None, kind: KindFilter::All }), ["e0", "a", "b"]);
    assert_eq!(ids(&events, Filter { day: None, hour: Some(1), kind: KindFilter::All }), ["b", "c", "d"]);
    assert_eq!(ids(&events, Filter { day: Some(2), hour: Some(1), kind: KindFilter::All }), ["c", "d"]);
    assert!(ids(&events, Filter { day: Some(3), hour: None, kind: KindFilter::All }).is_empty());
}

#[test]
fn the_combat_filter_keeps_only_combat_starts_and_ends_and_goes_with_the_day() {
    let mut events = a_day_and_a_bit();
    events.insert(3, combat_start("s", 3_500, Snapshot::default(), Snapshot::default()));
    events.insert(4, combat_end("x", 3_800, Outcome::Win));
    events.push(at(combat_start("s2", 6_000, Snapshot::default(), Snapshot::default()), Some(2), Some(2)));
    assert_eq!(ids(&events, Filter { day: None, hour: None, kind: KindFilter::Combat }), ["s", "x", "s2"]);
    assert_eq!(ids(&events, Filter { day: Some(2), hour: None, kind: KindFilter::Combat }), ["s2"]);
}

#[test]
fn a_filtered_list_keeps_the_runs_times_and_marks_its_first_row() {
    let rows = rows(&a_day_and_a_bit(), None, Filter { day: Some(2), hour: None, kind: KindFilter::All }, &Names::default());
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

#[test]
fn the_pvp_and_pve_filters_keep_the_starts_and_ends_of_those_combats() {
    let start = |id: &str, at: i64, is_pvp: bool| event(id, at, EventKind::CombatStart { player: Snapshot::default(), opponent: Snapshot::default(), is_pvp, opponent_name: None, opponent_hero: None, opponent_rating: None, opponent_level: None });
    let end_of = |id: &str, at: i64, start: Option<&str>| {
        let mut end = combat_end(id, at, Outcome::Win);
        if let EventKind::CombatEnd { combat_start_event_id, .. } = &mut end.kind {
            *combat_start_event_id = start.map(Into::into);
        }
        end
    };
    let mut events = a_day_and_a_bit();
    events.extend([start("monster", 7_000, false), end_of("monster-end", 7_100, Some("monster")), start("player", 7_200, true), end_of("player-end", 7_300, None)]);
    assert_eq!(ids(&events, Filter { day: None, hour: None, kind: KindFilter::Pve }), ["monster", "monster-end"]);
    assert_eq!(ids(&events, Filter { day: None, hour: None, kind: KindFilter::Pvp }), ["player", "player-end"]);
    assert_eq!(ids(&events, Filter { day: None, hour: None, kind: KindFilter::Combat }), ["monster", "monster-end", "player", "player-end"]);
}
