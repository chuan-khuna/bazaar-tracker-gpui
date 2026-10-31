//! Runs and Events. An Event is one thing that happened in a Run; a Run is one play-through.
//!
//! Events are our own JSON, not the original tracker's: snake_case keys, a `type` tag naming the
//! kind, and absent values left out. The fields every Event has (id, run, time, day and hour) come
//! first, then the kind's own.

use serde::{Deserialize, Serialize};

use crate::memory::Snapshot;

/// How a combat or a run ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    #[default]
    Unknown,
    Win,
    Loss,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Win => "Win",
            Self::Loss => "Loss",
        }
    }
}

/// One option the player was offered.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentedOption {
    pub instance_id: String,
    /// `Item`, `Encounter`, `EncounterStep`, `Skill`, `Combat` or `Unknown`, from the id's prefix.
    pub kind: String,
    pub size: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enchantment: Option<String>,
}

/// What kind of thing happened, and the details that go with it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventKind {
    /// The run began.
    RunStarted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        play_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        build_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        environment: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hero: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rating: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rank: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rank_division: Option<i32>,
    },
    /// The game offered cards, skills or encounters to choose from.
    OptionsPresented {
        /// The game state the options were offered in (`ChoiceState`, `LootState`, ...).
        context: String,
        options: Vec<PresentedOption>,
        snapshot: Snapshot,
    },
    /// The player took one of them.
    Selected {
        /// `Item`, `Skill`, `Encounter`, ...
        card_kind: String,
        instance_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        template_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tier: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        enchantment: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        section: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        /// Set when buying a duplicate upgraded an item the player had: the item it merged into.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        merged_into: Option<String>,
        /// The options Event this choice was made from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options_event_id: Option<String>,
        snapshot: Snapshot,
    },
    /// The player sold an item.
    Sold {
        instance_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        template_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tier: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        enchantment: Option<String>,
        snapshot: Snapshot,
    },
    /// The player paid to refresh the offers.
    Rerolled,
    /// A combat began, with both boards as they were just before it.
    CombatStart {
        player: Snapshot,
        opponent: Snapshot,
        is_pvp: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opponent_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opponent_hero: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opponent_rating: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opponent_level: Option<i32>,
    },
    /// A combat ended.
    CombatEnd {
        outcome: Outcome,
        /// The CombatStart Event this ends.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        combat_start_event_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        player_hp_after: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opponent_hp_after: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        victories_after: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        losses_after: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sim_winner: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sim_loser: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sim_frames: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_sim_frames: Option<i32>,
        /// Why the simulation's verdict was not used, if it was not.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sim_diag: Option<String>,
    },
    /// The run ended.
    RunEnded {
        outcome: Outcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        victories: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        losses: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gold: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        health_max: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prestige: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        income: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rating_after: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rank_after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rank_division_after: Option<i32>,
    },
}

impl EventKind {
    /// The `type` tag, as stored.
    pub fn name(&self) -> &'static str {
        match self {
            Self::RunStarted { .. } => "RunStarted",
            Self::OptionsPresented { .. } => "OptionsPresented",
            Self::Selected { .. } => "Selected",
            Self::Sold { .. } => "Sold",
            Self::Rerolled => "Rerolled",
            Self::CombatStart { .. } => "CombatStart",
            Self::CombatEnd { .. } => "CombatEnd",
            Self::RunEnded { .. } => "RunEnded",
        }
    }
}

/// One thing that happened in a Run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// A unique id, made when the Event was.
    pub id: String,
    /// The Run it belongs to: the game's run id, or a `local-` id until that is known.
    pub run_id: String,
    /// When it was made, in milliseconds since the Unix epoch.
    pub timestamp_ms: i64,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// A Run as listed in the history: who played it, how far it got and how it ended.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub started_ms: i64,
    pub ended_ms: Option<i64>,
    /// Whether the run has ended. A run that has not is the live run, or one the tracker lost.
    pub ended: bool,
    pub hero: Option<String>,
    pub day: Option<i32>,
    pub victories: Option<i32>,
    pub losses: Option<i32>,
    pub outcome: Option<Outcome>,
}

/// Whether this run id is one the tracker made up because the game's was not known yet.
pub fn is_local_run_id(id: &str) -> bool {
    id.starts_with("local-")
}

#[cfg(test)]
#[path = "event_tests.rs"]
mod tests;
