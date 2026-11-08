use bazaar_tracker::memory::{BoardState, PlayerStats, Snapshot};
use bazaar_tracker::run::{Event, EventKind, PresentedOption, Run};
use serde_json::json;

use super::*;
use crate::tracking::test_events::{item, options, snapshot};

fn run(ended: bool) -> Run {
    Run { id: "r1".into(), started_ms: 0, ended_ms: None, ended, hero: Some("Jules".into()), day: Some(5), victories: None, losses: None, outcome: None }
}

/// A shop on Day 5 Hour 2: one Medium item on slot 2, one in the stash, a skill, and two items
/// on offer.
fn shop() -> BoardState {
    let mut skill = item("skl_a", "Small", None);
    skill.card_type = "Skill".into();
    let mut enchanted = item("itm_board", "Medium", Some("Socket_2"));
    enchanted.enchantment = Some("Fiery".into());
    BoardState {
        player: Snapshot {
            board: vec![enchanted],
            stash: vec![item("itm_held", "Small", None)],
            skills: vec![skill],
            stats: PlayerStats { day: Some(5), hour: Some(2), gold: Some(12), level: Some(4), health: Some(900), health_max: Some(1200), hero: Some("Jules".into()), ..PlayerStats::default() },
        },
        run_state: Some("Encounter".into()),
        selection_set: vec!["itm_shop".into(), "gone".into()],
        entities: vec![item("itm_shop", "Large", None)],
        ..BoardState::default()
    }
}

#[test]
fn the_live_situation_comes_from_the_board_read() {
    let situation = from_board_state(&shop(), &Names::default());
    assert_eq!(
        situation.to_json(),
        json!({
            "hero": "Jules", "day": 5, "hour": 2, "gold": 12, "level": 4, "health": 900, "health_max": 1200,
            "game_state": "Shop",
            "board": [{"id": "tpl-itm_board", "name": "tpl-itm_board", "tier": "Gold", "size": "Medium", "slot": 2, "enchantment": "Fiery"}],
            "stash": [{"id": "tpl-itm_held", "name": "tpl-itm_held", "tier": "Gold", "size": "Small"}],
            "skills": [{"id": "tpl-skl_a", "name": "tpl-skl_a", "tier": "Gold", "size": "Small"}],
            // An id the read has no card for is left out.
            "offer": [{"id": "tpl-itm_shop", "name": "tpl-itm_shop", "tier": "Gold", "size": "Large", "kind": "merchant_item"}],
        })
    );
}

#[test]
fn offers_are_named_by_what_the_game_is_doing() {
    let mut board = shop();
    board.run_state = Some("Loot".into());
    assert_eq!(from_board_state(&board, &Names::default()).offer.unwrap()[0].kind, Some("loot"));
    let mut encounter = item("enc_a", "Medium", None);
    encounter.card_type = "CombatEncounter".into();
    let mut skill = item("skl_b", "Small", None);
    skill.card_type = "Skill".into();
    board.run_state = Some("Choice".into());
    board.selection_set = vec!["enc_a".into(), "skl_b".into()];
    board.entities = vec![encounter, skill];
    let situation = from_board_state(&board, &Names::default());
    assert_eq!(situation.game_state.as_deref(), Some("Encounter choice"));
    assert_eq!(situation.offer.unwrap().iter().map(|card| card.kind).collect::<Vec<_>>(), [Some("encounter_choice"), Some("skill")]);
}

#[test]
fn a_combat_sends_the_opponent() {
    let mut board = shop();
    board.run_state = Some("PVPCombat".into());
    board.selection_set.clear();
    board.opponent = Snapshot { board: vec![item("itm_them", "Small", Some("Socket_0"))], stats: PlayerStats { hero: Some("Mak".into()), health: Some(700), ..PlayerStats::default() }, ..Snapshot::default() };
    let situation = from_board_state(&board, &Names::default());
    assert_eq!(situation.game_state.as_deref(), Some("Combat"));
    assert_eq!(situation.offer, Some(vec![]));
    let opponent = situation.opponent.unwrap();
    assert_eq!((opponent.hero.as_deref(), opponent.health), (Some("Mak"), Some(700)));
    assert_eq!(opponent.board.iter().map(|card| (card.name.as_str(), card.slot)).collect::<Vec<_>>(), [("tpl-itm_them", Some(0))]);
}

#[test]
fn outside_a_combat_there_is_no_opponent() {
    let mut board = shop();
    board.opponent = Snapshot { board: vec![item("itm_them", "Small", None)], ..Snapshot::default() };
    assert_eq!(from_board_state(&board, &Names::default()).opponent, None);
}

#[test]
fn unknown_sizes_and_out_of_range_times_are_left_out() {
    let mut board = shop();
    board.player.skills[0].size = "7".into();
    board.player.stats.day = Some(0);
    board.player.stats.hour = Some(-1);
    let situation = from_board_state(&board, &Names::default());
    assert_eq!(situation.skills.unwrap()[0].size, None);
    assert_eq!((situation.day, situation.hour), (None, None));
}

#[test]
fn a_finished_run_sends_its_last_snapshot() {
    let mut last = snapshot(30, &["itm_a", "itm_b"]);
    last.stats.day = Some(9);
    let events = vec![options("e1", 1, snapshot(5, &["itm_a"])), options("e2", 2, last)];
    let situation = for_run(&run(true), false, Some(&shop()), &events, &Names::default());
    assert_eq!(
        situation.to_json(),
        json!({
            "hero": "Jules", "day": 9, "gold": 30, "game_state": "Run over",
            "board": [
                {"id": "tpl-itm_a", "name": "tpl-itm_a", "tier": "Gold", "size": "Small", "slot": 0},
                {"id": "tpl-itm_b", "name": "tpl-itm_b", "tier": "Gold", "size": "Small", "slot": 1},
            ],
            "stash": [], "skills": [],
        })
    );
}

#[test]
fn the_live_run_sends_the_board_read() {
    let events = vec![options("e1", 1, snapshot(5, &["itm_a"]))];
    let situation = for_run(&run(false), true, Some(&shop()), &events, &Names::default());
    assert_eq!(situation.game_state.as_deref(), Some("Shop"));
    assert_eq!(situation.gold, Some(12));
}

#[test]
fn the_live_run_before_a_read_sends_its_last_snapshot() {
    let events = vec![options("e1", 1, snapshot(5, &["itm_a"]))];
    let situation = for_run(&run(false), true, None, &events, &Names::default());
    assert_eq!((situation.gold, situation.game_state, situation.hero.as_deref()), (Some(5), None, Some("Jules")));
}

#[test]
fn a_run_without_a_snapshot_sends_its_hero_and_day() {
    let situation = for_run(&run(true), false, None, &[], &Names::default());
    assert_eq!(situation.to_json(), json!({"hero": "Jules", "day": 5, "game_state": "Run over"}));
}

fn offered(id: &str, kind: &str, size: &str) -> PresentedOption {
    PresentedOption { instance_id: id.into(), kind: kind.into(), size: size.into(), template_id: Some(format!("tpl-{id}")), tier: Some("Silver".into()), enchantment: None }
}

/// A finished Run: Options on Day 5 Hour 2 (an item, a skill, an encounter), the item picked from
/// them, a later Options, and a sale on Day 6.
fn timeline() -> Vec<Event> {
    use crate::tracking::test_events::event;
    let mut offer = event(
        "o1",
        60_000,
        EventKind::OptionsPresented { context: "ChoiceState".into(), options: vec![offered("itm_o", "Item", "Medium"), offered("skl_o", "Skill", "Small"), offered("enc_o", "Encounter", "")], snapshot: snapshot(7, &["itm_a"]) },
    );
    offer.day = Some(5);
    offer.hour = Some(2);
    let picked = event(
        "p1",
        61_000,
        EventKind::Selected {
            card_kind: "Item".into(),
            instance_id: "itm_o".into(),
            template_id: Some("tpl-itm_o".into()),
            tier: Some("Silver".into()),
            enchantment: None,
            section: None,
            target: None,
            merged_into: None,
            options_event_id: Some("o1".into()),
            snapshot: snapshot(3, &["itm_a", "itm_o"]),
        },
    );
    let loot = event("o2", 62_000, EventKind::OptionsPresented { context: "LootState".into(), options: vec![offered("itm_l", "Item", "Small")], snapshot: snapshot(3, &["itm_a", "itm_o"]) });
    let mut sale = event("s1", 125_000, EventKind::Sold { instance_id: "itm_a".into(), template_id: Some("tpl-itm_a".into()), tier: Some("Gold".into()), enchantment: Some("Icy".into()), snapshot: snapshot(9, &["itm_o"]) });
    sale.day = Some(6);
    sale.hour = Some(0);
    vec![offer, picked, loot, sale]
}

#[test]
fn options_become_the_offer_and_say_what_was_picked() {
    let situation = for_event(&run(true), &timeline(), "o1", &Names::default()).unwrap();
    assert_eq!(
        situation.to_json(),
        json!({
            // The Snapshot at that Event, at its Day and Hour; not "Run over", as it is that moment.
            "hero": "Jules", "day": 5, "hour": 2, "gold": 7, "game_state": "Choice",
            "board": [{"id": "tpl-itm_a", "name": "tpl-itm_a", "tier": "Gold", "size": "Small", "slot": 0}],
            "stash": [], "skills": [],
            "offer": [
                {"id": "tpl-itm_o", "name": "tpl-itm_o", "tier": "Silver", "size": "Medium", "kind": "merchant_item"},
                {"id": "tpl-skl_o", "name": "tpl-skl_o", "tier": "Silver", "size": "Small", "kind": "skill"},
                {"id": "tpl-enc_o", "name": "tpl-enc_o", "tier": "Silver", "kind": "encounter_choice"},
            ],
            "event": {
                "id": "o1", "number": 1, "kind": "Options", "time": "+0:00", "detail": "3 options · Choice",
                "picked": {"id": "tpl-itm_o", "name": "tpl-itm_o", "tier": "Silver", "size": "Medium"},
            },
        })
    );
}

#[test]
fn loot_options_are_loot_and_unpicked_options_pick_nothing() {
    let situation = for_event(&run(true), &timeline(), "o2", &Names::default()).unwrap();
    assert_eq!(situation.offer.unwrap()[0].kind, Some("loot"));
    let event = situation.event.unwrap();
    assert_eq!((event.number, event.picked, event.time.as_deref()), (3, None, Some("+0:02")));
    // The Day and Hour carry over from the Event before.
    assert_eq!((situation.day, situation.hour), (Some(5), Some(2)));
}

#[test]
fn a_selected_event_says_what_was_taken() {
    let situation = for_event(&run(true), &timeline(), "p1", &Names::default()).unwrap();
    assert_eq!(situation.offer, None);
    assert_eq!(situation.gold, Some(3));
    let event = situation.event.unwrap();
    assert_eq!((event.kind.as_str(), event.sold), ("Selected", None));
    // Its size comes from the Options it was taken from.
    assert_eq!(event.picked, Some(Card { id: Some("tpl-itm_o".into()), name: "tpl-itm_o".into(), tier: Some("Silver".into()), size: Some("Medium".into()), ..Card::default() }));
}

#[test]
fn a_sold_event_says_what_was_sold() {
    let situation = for_event(&run(true), &timeline(), "s1", &Names::default()).unwrap();
    assert_eq!((situation.day, situation.hour, situation.gold), (Some(6), Some(0), Some(9)));
    let event = situation.event.unwrap();
    assert_eq!(event.sold, Some(Card { id: Some("tpl-itm_a".into()), name: "tpl-itm_a".into(), tier: Some("Gold".into()), enchantment: Some("Icy".into()), ..Card::default() }));
    assert_eq!(event.picked, None);
    assert_eq!(for_event(&run(true), &timeline(), "gone", &Names::default()), None);
}
