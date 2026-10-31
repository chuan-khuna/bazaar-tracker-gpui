//! What the tracker knows of the board at one moment: the Snapshot saved with an Event.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One card: an item, a skill or an encounter. Names are the game's enum names; a number the
/// tracker has no name for is kept as its digits.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// The id of this copy, such as `itm_EuUtMwz`. Its prefix says what kind of card it is.
    pub instance_id: String,
    /// The id of the card it is a copy of.
    pub template_id: Option<String>,
    /// `Item`, `Skill`, `CombatEncounter` and so on.
    pub card_type: String,
    pub tier: String,
    pub size: String,
    /// Which of the board's sockets it sits in, if it sits in one.
    pub socket: Option<String>,
    pub enchantment: Option<String>,
    /// Attribute name to value, such as `DamageAmount` to 30.
    pub attributes: BTreeMap<String, i32>,
    pub tags: Vec<String>,
}

/// The player's numbers. Anything the game had not set yet is `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerStats {
    pub health: Option<i32>,
    pub health_max: Option<i32>,
    pub gold: Option<i32>,
    pub prestige: Option<i32>,
    pub level: Option<i32>,
    pub income: Option<i32>,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub victories: Option<i32>,
    pub losses: Option<i32>,
    pub hero: Option<String>,
}

/// The board, stash and skills of one side, with that side's stats, as they were at one moment.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub board: Vec<Item>,
    pub stash: Vec<Item>,
    pub skills: Vec<Item>,
    pub stats: PlayerStats,
}

/// Everything one board read gives: both sides' Snapshots, and what the run state machine needs to
/// know about the moment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoardState {
    pub player: Snapshot,
    pub opponent: Snapshot,
    /// The game's run state by name (`Choice`, `Combat`, `EndRunVictory`, ...).
    pub run_state: Option<String>,
    /// The ids of the cards on offer right now.
    pub selection_set: Vec<String>,
    pub is_storage_open: bool,
    /// Every card the read saw, on either side or on offer: the source for looking up a card's
    /// tier, size and enchantment from its id.
    pub entities: Vec<Item>,
}

impl BoardState {
    /// The card with this instance id, among everything the read saw.
    pub fn entity(&self, instance_id: &str) -> Option<&Item> {
        self.entities.iter().find(|item| item.instance_id == instance_id)
    }

    /// The ids of every card the player has on the board, in the stash or as a skill.
    pub fn player_item_ids(&self) -> Vec<String> {
        let player = &self.player;
        player.board.iter().chain(&player.stash).chain(&player.skills).map(|item| item.instance_id.clone()).collect()
    }
}
