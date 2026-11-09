use serde_json::json;

use super::*;

fn names() -> Names {
    Names::default()
}

/// A shop on Day 14 Hour 6 with a full Board, as the app sends it.
fn shop() -> Value {
    json!({
        "hero": "Jules",
        "day": 14,
        "hour": 6,
        "gold": 12,
        "level": 7,
        "health": 900,
        "health_max": 1200,
        "game_state": "Shop",
        "board": [
            {"id": "itm_a", "name": "Cutlass", "tier": "Gold", "size": "Medium", "slot": 0, "enchantment": "Fiery"},
            {"id": "itm_b", "name": "Anvil", "tier": "Silver", "size": "Large", "slot": 2},
            {"id": "itm_c", "name": "Pan", "tier": "Bronze", "size": "Large", "slot": 5},
            {"id": "itm_d", "name": "Spoon", "size": "Medium", "slot": 8}
        ],
        "stash": [{"id": "itm_e", "name": "Lamp", "tier": "Bronze", "size": "Small"}, {"name": "Rope", "size": "Medium"}],
        "skills": [{"id": "skl_a", "name": "Hot Hands", "tier": "Silver"}],
        "offer": [
            {"id": "itm_f", "name": "Whisk", "tier": "Diamond", "size": "Small", "kind": "merchant_item"},
            {"name": "Pick a path", "kind": "encounter_choice"}
        ]
    })
}

#[test]
fn the_summary_names_the_day_hour_and_board_slots() {
    assert_eq!(view(Some(&shop()), &names()).summary, "Context · D14 H6 · 10/10 slots");
    assert_eq!(view(Some(&json!({"day": 3, "board": []})), &names()).summary, "Context · D3 · 0/10 slots");
    assert_eq!(view(Some(&json!({})), &names()).summary, "Context");
}

#[test]
fn the_players_numbers_are_listed_in_order() {
    let sent = view(Some(&shop()), &names()).sent.unwrap();
    assert_eq!(
        sent.facts,
        [("Hero", "Jules".to_owned()), ("Time", "D14 H6".to_owned()), ("Gold", "12".to_owned()), ("Level", "7".to_owned()), ("Health", "900/1200".to_owned()), ("State", "Shop".to_owned())]
    );
}

#[test]
fn the_board_stash_skills_and_offer_are_laid_out_with_their_sent_names() {
    let sent = view(Some(&shop()), &names()).sent.unwrap();
    let board = sent.board.unwrap();
    let laid: Vec<(u8, u8, &str)> = board.tiles.iter().map(|tile| (tile.start, tile.span, tile.name.as_str())).collect();
    assert_eq!(laid, [(0, 2, "Cutlass"), (2, 3, "Anvil"), (5, 3, "Pan"), (8, 2, "Spoon")]);
    assert_eq!(board.tiles[0].tier, Some(Tier::Gold));
    assert_eq!(board.tiles[0].enchantment.as_deref(), Some("Fiery"));
    assert_eq!(board.tiles[3].tier, None);

    let stash = sent.stash.unwrap();
    let laid: Vec<(u8, u8, &str)> = stash.tiles.iter().map(|tile| (tile.start, tile.span, tile.name.as_str())).collect();
    assert_eq!(laid, [(0, 1, "Lamp"), (1, 2, "Rope")]);

    let skills = sent.skills.unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!((skills[0].name.as_str(), skills[0].tier), ("Hot Hands", Some(Tier::Silver)));

    let offer = sent.offer.unwrap();
    assert_eq!(offer.iter().map(|card| (card.name.as_str(), card.kind.as_str())).collect::<Vec<_>>(), [("Whisk", "merchant item"), ("Pick a path", "encounter choice")]);
    assert_eq!(offer[0].tier, Some(Tier::Diamond));
    assert!(sent.opponent.is_none());
}

#[test]
fn the_json_is_the_situation_as_sent() {
    let situation = shop();
    let sent = view(Some(&situation), &names()).sent.unwrap();
    assert_eq!(serde_json::from_str::<Value>(&sent.json).unwrap(), situation);
}

#[test]
fn missing_fields_are_left_out() {
    let sent = view(Some(&json!({"hero": "Vanessa", "health": 300, "board": "not a list"})), &names()).sent.unwrap();
    assert_eq!(sent.facts, [("Hero", "Vanessa".to_owned()), ("Health", "300".to_owned())]);
    assert!(sent.board.is_none() && sent.stash.is_none() && sent.skills.is_none() && sent.offer.is_none() && sent.opponent.is_none());
}

#[test]
fn the_opponent_is_shown_in_a_combat() {
    let situation = json!({
        "day": 5,
        "game_state": "Combat",
        "offer": [],
        "opponent": {"hero": "Mak", "health": 400, "health_max": 500, "board": [{"id": "itm_x", "name": "Dagger", "size": "Small", "slot": 4}], "skills": []}
    });
    let opponent = view(Some(&situation), &names()).sent.unwrap().opponent.unwrap();
    assert_eq!(opponent.facts, [("Hero", "Mak".to_owned()), ("Health", "400/500".to_owned())]);
    assert_eq!(opponent.board.tiles.iter().map(|tile| (tile.start, tile.name.as_str())).collect::<Vec<_>>(), [(4, "Dagger")]);
    assert!(opponent.skills.is_empty());
}

#[test]
fn a_question_sent_without_a_situation_says_so() {
    for none in [None, Some(&Value::Null)] {
        let context = view(none, &names());
        assert_eq!(context.summary, "Context · none sent");
        assert!(context.sent.is_none());
    }
}

#[test]
fn the_situation_the_app_builds_reads_back() {
    use super::super::situation::{Card, Situation};
    let situation = Situation {
        day: Some(2),
        hour: Some(1),
        board: Some(vec![Card { id: Some("itm_a".into()), name: "Cutlass".into(), size: Some("Medium".into()), slot: Some(3), ..Card::default() }]),
        ..Situation::default()
    };
    let context = view(Some(&situation.to_json()), &names());
    assert_eq!(context.summary, "Context · D2 H1 · 2/10 slots");
    assert_eq!(context.sent.unwrap().board.unwrap().tiles[0].start, 3);
}

#[test]
fn an_attached_event_is_in_the_summary_and_first_among_the_facts() {
    let mut situation = shop();
    situation["event"] = json!({"id": "e7", "number": 7, "kind": "Options", "time": "+12:05", "detail": "3 options · Choice", "picked": {"id": "itm_f", "name": "Whisk", "enchantment": "Icy"}});
    let view = view(Some(&situation), &names());
    assert_eq!(view.summary, "Context · Event #7 · D14 H6 · 10/10 slots");
    let facts = view.sent.unwrap().facts;
    assert_eq!(facts[..3], [("Event", "#7 Options: 3 options · Choice (+12:05)".to_owned()), ("Picked", "Icy Whisk".to_owned()), ("Hero", "Jules".to_owned())]);

    let sold = json!({"event": {"id": "e8", "number": 8, "kind": "Sold", "sold": {"name": "Anvil"}}});
    assert_eq!(view_facts(&sold), [("Event", "#8 Sold".to_owned()), ("Sold", "Anvil".to_owned())]);
}

fn view_facts(situation: &Value) -> Vec<(&'static str, String)> {
    view(Some(situation), &names()).sent.unwrap().facts
}

#[test]
fn slot_effects_are_marked_on_the_board() {
    let mut situation = shop();
    situation["slot_effects"] = json!([{"slot": 3, "effect": "heat", "name": "Stove"}, {"slot": 8, "effect": "note", "name": "B Note (Burn)"}]);
    let board = view(Some(&situation), &names()).sent.unwrap().board.unwrap();
    assert_eq!(board.marks.iter().map(|mark| (mark.slot, mark.label.as_str())).collect::<Vec<_>>(), [(3, "Heat"), (8, "B")]);
    assert_eq!(board.tiles.len(), 4);
}
