//! The situation: the JSON sent with every message, saying where the player is in the Run.
//!
//! For the Live run it is built from the latest board read (the one the Live section's `LiveView`
//! is drawn from): the game's state, the Offer and, in a combat, the opponent. For a finished Run
//! (or the Live run before the game has been read) it is built from the Run's last Snapshot. The
//! schema is checked by the Advisor (`agents/advisor/situation.py`, documented in
//! docs/documents/advisor.md "The situation"): a field left out is not known, and an unknown field
//! is refused, so only the schema's fields are written.

use bazaar_tracker::memory::{BoardState, Item, Snapshot};
use bazaar_tracker::run::{Event, Run};
use serde::Serialize;

use crate::tracking::live::state_words;
use crate::tracking::names::Names;
use crate::tracking::run::board::{Grid, board_grid, stash_grid};
use crate::tracking::run::moment;

/// One card, as the schema has it. `kind` is only on an Offer's cards.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Card {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enchantment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
}

/// The opponent in a combat.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Opponent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hero: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_max: Option<i32>,
    pub board: Vec<Card>,
    pub skills: Vec<Card>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Situation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hero: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hour: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gold: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_max: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_state: Option<String>,
    /// `None`: not known (no Snapshot yet). `Some(vec![])`: empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<Vec<Card>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stash: Option<Vec<Card>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<Card>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer: Option<Vec<Card>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent: Option<Opponent>,
}

impl Situation {
    /// As the JSON the API takes.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// The situation to send for `run`. The Live run uses the latest board read when there is one
/// (`board`); otherwise, and for any other Run, its last Snapshot among `events` (the Run's Events
/// in order). With neither, only what the Run's row knows: the hero and the day.
pub fn for_run(run: &Run, live: bool, board: Option<&BoardState>, events: &[Event], names: &Names) -> Situation {
    if live && let Some(board) = board {
        let mut situation = from_board_state(board, names);
        situation.hero = situation.hero.or_else(|| run.hero.clone());
        return situation;
    }
    match moment::at(events, None) {
        Some(snapshot) => from_snapshot(snapshot, run, names),
        None => Situation { hero: run.hero.clone(), day: run.day.filter(|day| *day >= 1), game_state: run.ended.then(|| "Run over".to_owned()), ..Situation::default() },
    }
}

/// The situation in one board read of the Live run: the player's numbers, Board, Stash and skills,
/// the game's state, what is on offer and, in a combat, the opponent.
pub fn from_board_state(board: &BoardState, names: &Names) -> Situation {
    let (words, _) = state_words(board.run_state.as_deref());
    let mut situation = player(&board.player, names);
    situation.game_state = Some(words);
    let offer: Vec<Card> = board.selection_set.iter().filter_map(|id| board.entity(id)).map(|item| Card { kind: Some(offer_kind(item, board.run_state.as_deref())), ..card(item, names) }).collect();
    situation.offer = Some(offer);
    let combat = matches!(board.run_state.as_deref(), Some("Combat" | "PVPCombat"));
    let them = &board.opponent;
    if combat && !(them.board.is_empty() && them.skills.is_empty()) {
        situation.opponent = Some(Opponent {
            hero: them.stats.hero.clone(),
            health: them.stats.health,
            health_max: them.stats.health_max,
            board: board_cards(&board_grid(&them.board, names)),
            skills: them.skills.iter().map(|skill| card(skill, names)).collect(),
        });
    }
    situation
}

/// The situation at a saved Snapshot of `run`: the player's numbers, Board, Stash and skills. A
/// Run that has ended says so as its game state; nothing is on offer.
pub fn from_snapshot(snapshot: &Snapshot, run: &Run, names: &Names) -> Situation {
    let mut situation = player(snapshot, names);
    situation.hero = situation.hero.or_else(|| run.hero.clone());
    situation.day = situation.day.or(run.day.filter(|day| *day >= 1));
    if run.ended {
        situation.game_state = Some("Run over".to_owned());
    }
    situation
}

/// The player's side of a Snapshot.
fn player(snapshot: &Snapshot, names: &Names) -> Situation {
    let stats = &snapshot.stats;
    Situation {
        hero: stats.hero.clone(),
        // The schema wants a day from 1 and an hour from 0.
        day: stats.day.filter(|day| *day >= 1),
        hour: stats.hour.filter(|hour| *hour >= 0),
        gold: stats.gold,
        level: stats.level,
        health: stats.health,
        health_max: stats.health_max,
        board: Some(board_cards(&board_grid(&snapshot.board, names))),
        stash: Some(stash_grid(&snapshot.stash, names).tiles.iter().map(|tile| Card { slot: None, ..tile_card(tile) }).collect()),
        skills: Some(snapshot.skills.iter().map(|skill| card(skill, names)).collect()),
        ..Situation::default()
    }
}

/// The Board's items where the board draws them, each with its first slot. `SocketEffect` cards
/// are not items and are left out, as the board leaves them out.
fn board_cards(grid: &Grid) -> Vec<Card> {
    grid.tiles.iter().map(tile_card).collect()
}

fn tile_card(tile: &crate::tracking::run::board::Tile) -> Card {
    Card {
        id: tile.template_id.clone(),
        name: tile.name.clone(),
        tier: tier(&tile.tier_name),
        size: Some(match tile.span {
            2 => "Medium",
            3 => "Large",
            _ => "Small",
        }
        .to_owned()),
        slot: Some(tile.start),
        enchantment: tile.enchantment.clone(),
        kind: None,
    }
}

/// A card read from the game: its id, name, tier, size (when the game named one the schema knows)
/// and enchantment.
fn card(item: &Item, names: &Names) -> Card {
    Card {
        id: item.template_id.clone(),
        name: names.card(item.template_id.as_deref(), &item.instance_id),
        tier: tier(&item.tier),
        size: matches!(item.size.as_str(), "Small" | "Medium" | "Large").then(|| item.size.clone()),
        slot: None,
        enchantment: item.enchantment.clone(),
        kind: None,
    }
}

/// A tier the game named; an empty one is not known.
fn tier(name: &str) -> Option<String> {
    (!name.is_empty()).then(|| name.to_owned())
}

/// What a card on offer is: a skill is a skill; in a merchant's shop an item is the merchant's; at
/// an encounter choice anything else is an encounter; elsewhere (after a fight) an item is loot.
fn offer_kind(item: &Item, run_state: Option<&str>) -> &'static str {
    match (item.card_type.as_str(), run_state) {
        ("Skill", _) => "skill",
        ("Item", Some("Encounter")) => "merchant_item",
        ("Item", _) => "loot",
        _ => "encounter_choice",
    }
}

#[cfg(test)]
#[path = "situation_tests.rs"]
mod tests;
