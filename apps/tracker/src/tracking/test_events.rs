//! Events and Snapshots for the tests of the view models.

use bazaar_tracker::memory::{Item, PlayerStats, Snapshot};
use bazaar_tracker::run::{Event, EventKind, Outcome};

pub fn event(id: &str, at: i64, kind: EventKind) -> Event {
    Event { id: id.into(), run_id: "r1".into(), timestamp_ms: at, day: None, hour: None, kind }
}

/// An item on the board.
pub fn item(instance_id: &str, size: &str, socket: Option<&str>) -> Item {
    Item {
        instance_id: instance_id.into(),
        template_id: Some(format!("tpl-{instance_id}")),
        card_type: "Item".into(),
        tier: "Gold".into(),
        size: size.into(),
        socket: socket.map(str::to_owned),
        ..Item::default()
    }
}

/// A Snapshot whose board is made of these item ids (all Small, unplaced) and whose gold says
/// which Snapshot it is.
pub fn snapshot(gold: i32, board: &[&str]) -> Snapshot {
    Snapshot { board: board.iter().map(|id| item(id, "Small", None)).collect(), stats: PlayerStats { gold: Some(gold), ..PlayerStats::default() }, ..Snapshot::default() }
}

pub fn options(id: &str, at: i64, snapshot: Snapshot) -> Event {
    event(id, at, EventKind::OptionsPresented { context: "ChoiceState".into(), options: vec![], snapshot })
}

pub fn sold(id: &str, at: i64, snapshot: Snapshot) -> Event {
    event(id, at, EventKind::Sold { instance_id: "itm_x".into(), template_id: None, tier: None, enchantment: None, snapshot })
}

pub fn combat_end(id: &str, at: i64, outcome: Outcome) -> Event {
    event(
        id,
        at,
        EventKind::CombatEnd {
            outcome,
            combat_start_event_id: None,
            player_hp_after: None,
            opponent_hp_after: None,
            victories_after: None,
            losses_after: None,
            sim_winner: None,
            sim_loser: None,
            sim_frames: None,
            expected_sim_frames: None,
            sim_diag: None,
        },
    )
}

pub fn combat_start(id: &str, at: i64, player: Snapshot, opponent: Snapshot) -> Event {
    event(id, at, EventKind::CombatStart { player, opponent, is_pvp: true, opponent_name: None, opponent_hero: None, opponent_rating: None, opponent_level: None })
}

pub fn gold_of(snapshot: Option<&Snapshot>) -> Option<i32> {
    snapshot.and_then(|snapshot| snapshot.stats.gold)
}
