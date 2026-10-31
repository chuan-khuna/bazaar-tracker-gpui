//! The trait the run state machine reads game state through. The real implementation reads The
//! Bazaar's memory through `unityspy-rs`; [`fake::FakeMemory`] returns scripted values for tests.
//!
//! Every read is `Ok(None)` while the game has not got a value yet ("not available yet", which is
//! normal and never an error), `Ok(Some(..))` when it has, and `Err` only when reading failed.

pub mod fake;
#[cfg(windows)]
pub mod game;
pub mod snapshot;

use std::fmt;

pub use snapshot::{BoardState, Item, PlayerStats, Snapshot};

/// Why a read failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryError {
    /// The game is not attached.
    Disconnected,
    /// The game is attached but the read went wrong.
    Failed(String),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "the game is not attached"),
            Self::Failed(why) => write!(f, "reading the game failed: {why}"),
        }
    }
}

impl std::error::Error for MemoryError {}

pub type MemoryResult<T> = Result<Option<T>, MemoryError>;

/// A change in whether the game is attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionChange {
    Connected { pid: u32, unity_version: String },
    Disconnected { reason: String },
}

/// What the game says about the run that just began. The id is the real run id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunInit {
    pub run_id: Option<String>,
    pub play_mode: Option<String>,
    pub build_id: Option<String>,
    pub environment: Option<String>,
}

/// How the last combat simulation went: who won, and how many frames it has.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatResult {
    /// The combatant id of the winner: 0 is the player, 1 the opponent.
    pub winner: Option<i32>,
    pub loser: Option<i32>,
    pub frame_count: Option<i32>,
}

/// The size of the last combat simulation's frame list, used to tell a stale simulation from the
/// one just played. The frames themselves are not read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatFrames {
    pub frame_count: i32,
}

/// The PvP opponent of the current fight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PvpOpponent {
    pub name: Option<String>,
    pub hero: Option<String>,
    pub rating: Option<i32>,
    pub level: Option<i32>,
}

/// The player's ranked standing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerRank {
    pub rating: i32,
    /// The rank by name (`Gold`).
    pub rank: String,
    pub division: i32,
}

/// The reads the run state machine makes. One method for each.
pub trait GameMemory: Send {
    /// Both sides' boards, stash and skills and the player's stats.
    fn read_board(&mut self) -> MemoryResult<BoardState>;
    /// The run that just began, with its real id.
    fn read_run_init(&mut self) -> MemoryResult<RunInit>;
    /// The winner and loser of the last combat simulation.
    fn read_combat_result(&mut self) -> MemoryResult<CombatResult>;
    /// The size of the last combat simulation.
    fn read_combat_frames(&mut self) -> MemoryResult<CombatFrames>;
    /// The opponent of the current PvP fight.
    fn read_pvp_opponent(&mut self) -> MemoryResult<PvpOpponent>;
    /// The hero picked for the run, by name.
    fn read_selected_hero(&mut self) -> MemoryResult<String>;
    /// The player's rating, rank and division.
    fn read_player_rank(&mut self) -> MemoryResult<PlayerRank>;
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
