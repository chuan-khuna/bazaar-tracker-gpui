use std::collections::BTreeMap;

use super::*;
use crate::memory::{Item, PlayerStats};

fn snapshot() -> Snapshot {
    Snapshot {
        board: vec![Item {
            instance_id: "itm_EuUtMwz".into(),
            template_id: Some("638a6b50-7803-4da5-9514-ca0627fd7b5f".into()),
            card_type: "Item".into(),
            tier: "Silver".into(),
            size: "Medium".into(),
            socket: Some("Socket_2".into()),
            enchantment: Some("Fiery".into()),
            attributes: BTreeMap::from([("DamageAmount".to_owned(), 30), ("Cooldown".to_owned(), 4000)]),
            tags: vec!["Weapon".into(), "Tool".into()],
        }],
        stash: vec![],
        skills: vec![Item { instance_id: "skl_x".into(), card_type: "Skill".into(), tier: "Bronze".into(), size: "Small".into(), ..Item::default() }],
        stats: PlayerStats { health: Some(300), health_max: Some(300), gold: Some(12), day: Some(2), hour: Some(3), victories: Some(1), losses: Some(0), hero: Some("Jules".into()), ..PlayerStats::default() },
    }
}

fn event(kind: EventKind) -> Event {
    Event { id: "e1".into(), run_id: "run-1".into(), timestamp_ms: 1_761_900_000_123, day: Some(2), hour: Some(3), kind }
}

fn every_kind() -> Vec<EventKind> {
    vec![
        EventKind::RunStarted {
            play_mode: Some("Ranked".into()),
            build_id: Some("b1".into()),
            environment: Some("prod".into()),
            hero: Some("Jules".into()),
            rating: Some(1234),
            rank: Some("Gold".into()),
            rank_division: Some(2),
        },
        EventKind::OptionsPresented {
            context: "ChoiceState".into(),
            options: vec![
                PresentedOption { instance_id: "itm_a".into(), kind: "Item".into(), size: "Small".into(), template_id: Some("t".into()), tier: Some("Bronze".into()), enchantment: None },
                PresentedOption { instance_id: "enc_b".into(), kind: "Encounter".into(), size: "Unknown".into(), ..PresentedOption::default() },
            ],
            snapshot: snapshot(),
        },
        EventKind::Selected {
            card_kind: "Item".into(),
            instance_id: "itm_a".into(),
            template_id: Some("t".into()),
            tier: Some("Bronze".into()),
            enchantment: None,
            section: Some("Player".into()),
            target: Some("PlayerSocket_3".into()),
            merged_into: None,
            options_event_id: Some("e0".into()),
            snapshot: snapshot(),
        },
        EventKind::Sold { instance_id: "itm_a".into(), template_id: None, tier: Some("Bronze".into()), enchantment: None, snapshot: snapshot() },
        EventKind::Rerolled,
        EventKind::CombatStart {
            player: snapshot(),
            opponent: Snapshot::default(),
            is_pvp: true,
            opponent_name: Some("Rival".into()),
            opponent_hero: Some("Vanessa".into()),
            opponent_rating: Some(1180),
            opponent_level: Some(9),
        },
        EventKind::CombatEnd {
            outcome: Outcome::Win,
            combat_start_event_id: Some("e5".into()),
            player_hp_after: Some(210),
            opponent_hp_after: Some(0),
            victories_after: Some(3),
            losses_after: Some(1),
            sim_winner: Some(0),
            sim_loser: Some(1),
            sim_frames: Some(659),
            expected_sim_frames: Some(659),
            sim_diag: None,
        },
        EventKind::RunEnded {
            outcome: Outcome::Loss,
            victories: Some(4),
            losses: Some(3),
            gold: Some(17),
            health_max: Some(900),
            prestige: Some(20),
            level: Some(14),
            income: Some(12),
            rating_after: Some(1262),
            rank_after: Some("Gold".into()),
            rank_division_after: Some(1),
        },
    ]
}

#[test]
fn every_kind_of_event_survives_a_json_round_trip() {
    for kind in every_kind() {
        let event = event(kind);
        let json = serde_json::to_string(&event).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(back, event, "{json}");
    }
}

#[test]
fn an_event_is_flat_json_with_a_type_tag_and_snake_case_keys() {
    let json = serde_json::to_value(event(EventKind::Rerolled)).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"id": "e1", "run_id": "run-1", "timestamp_ms": 1_761_900_000_123i64, "day": 2, "hour": 3, "type": "Rerolled"})
    );
    let selected = serde_json::to_value(event(every_kind().remove(2))).unwrap();
    assert_eq!(selected["type"], "Selected");
    assert_eq!(selected["options_event_id"], "e0");
    assert_eq!(selected["snapshot"]["board"][0]["instance_id"], "itm_EuUtMwz");
    assert_eq!(selected["snapshot"]["stats"]["health_max"], 300);
}

#[test]
fn absent_values_are_left_out_not_written_as_null() {
    let json = serde_json::to_value(event(EventKind::Sold { instance_id: "itm_a".into(), template_id: None, tier: None, enchantment: None, snapshot: Snapshot::default() })).unwrap();
    let object = json.as_object().unwrap();
    assert!(!object.contains_key("template_id") && !object.contains_key("tier"));
    assert!(object.contains_key("snapshot"));
}

#[test]
fn an_event_with_a_missing_day_reads_back_without_one() {
    let mut original = event(EventKind::Rerolled);
    original.day = None;
    original.hour = None;
    let back: Event = serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
    assert_eq!((back.day, back.hour), (None, None));
}

#[test]
fn kinds_name_themselves_by_their_tag() {
    for kind in every_kind() {
        let json = serde_json::to_value(&kind).unwrap();
        assert_eq!(json["type"], kind.name());
    }
}

#[test]
fn only_ids_with_the_local_prefix_are_local() {
    assert!(is_local_run_id("local-0d4c2b9e8f7a4c1b9a3e5d6f7a8b9c0d"));
    assert!(!is_local_run_id("3f2a9c1e-77aa"));
    assert!(!is_local_run_id(""));
}
