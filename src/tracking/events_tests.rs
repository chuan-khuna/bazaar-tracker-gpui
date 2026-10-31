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
    rows(events, None, &Names::default())
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
fn the_day_is_shown_only_when_it_changes() {
    let events = [
        at(started(), Some(1), Some(1)),
        at(event("a", 0, EventKind::Rerolled), Some(1), Some(2)),
        at(event("b", 0, EventKind::Rerolled), Some(1), Some(2)),
        at(event("c", 0, EventKind::Rerolled), Some(2), Some(1)),
        at(event("d", 0, EventKind::Rerolled), None, None),
        at(event("e", 0, EventKind::Rerolled), Some(2), Some(1)),
    ];
    let labels: Vec<_> = rows_of(&events).into_iter().map(|row| row.hour).collect();
    assert_eq!(labels, ["D1 H1", "H2", "", "D2 H1", "", ""]);
}

#[test]
fn the_selected_event_is_marked() {
    let events = [started(), event("a", 0, EventKind::Rerolled)];
    let rows = rows(&events, Some("a"), &Names::default());
    assert_eq!(rows.iter().map(|row| row.selected).collect::<Vec<_>>(), [false, true]);
}
