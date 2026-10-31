//! A game memory that returns scripted values, for tests of the run state machine, the tracker
//! service and anything else that reads the game.
//!
//! Each read has its own script. `push_*` queues a result for the next call; once the queue is
//! used up the last result is repeated, and a read nothing was scripted for is `Ok(None)` ("not
//! available yet"). `set_*` replaces the script with one result that repeats. A `FakeMemory` is
//! cheap to clone and every clone shares the scripts, so a test can change what the game says
//! after the memory has been handed to the code under test.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::{BoardState, CombatFrames, CombatResult, GameMemory, MemoryError, MemoryResult, PlayerRank, PvpOpponent, RunInit};

struct Script<T> {
    queue: VecDeque<MemoryResult<T>>,
    last: Option<MemoryResult<T>>,
    calls: usize,
}

impl<T> Default for Script<T> {
    fn default() -> Self {
        Self { queue: VecDeque::new(), last: None, calls: 0 }
    }
}

impl<T: Clone> Script<T> {
    fn next(&mut self) -> MemoryResult<T> {
        self.calls += 1;
        if let Some(result) = self.queue.pop_front() {
            self.last = Some(result.clone());
            return result;
        }
        self.last.clone().unwrap_or(Ok(None))
    }

    fn set(&mut self, result: MemoryResult<T>) {
        self.queue.clear();
        self.last = Some(result);
    }
}

#[derive(Default)]
struct Scripts {
    board: Script<BoardState>,
    run_init: Script<RunInit>,
    combat_result: Script<CombatResult>,
    combat_frames: Script<CombatFrames>,
    pvp_opponent: Script<PvpOpponent>,
    selected_hero: Script<String>,
    player_rank: Script<PlayerRank>,
}

/// A scriptable [`GameMemory`].
#[derive(Clone, Default)]
pub struct FakeMemory {
    scripts: Arc<Mutex<Scripts>>,
}

macro_rules! scripted {
    ($field:ident, $ty:ty, $set:ident, $push:ident, $fail:ident, $calls:ident) => {
        /// Answer every call with this value (or `None`: not available yet) from now on.
        pub fn $set(&self, value: impl Into<Option<$ty>>) {
            self.scripts.lock().unwrap().$field.set(Ok(value.into()));
        }

        /// Answer the next call (after any already queued) with this value.
        pub fn $push(&self, value: impl Into<Option<$ty>>) {
            self.scripts.lock().unwrap().$field.queue.push_back(Ok(value.into()));
        }

        /// Answer every call with an error from now on.
        pub fn $fail(&self, why: &str) {
            self.scripts.lock().unwrap().$field.set(Err(MemoryError::Failed(why.to_owned())));
        }

        /// How many times this has been read.
        pub fn $calls(&self) -> usize {
            self.scripts.lock().unwrap().$field.calls
        }
    };
}

impl FakeMemory {
    pub fn new() -> Self {
        Self::default()
    }

    scripted!(board, BoardState, set_board, push_board, fail_board, board_calls);
    scripted!(run_init, RunInit, set_run_init, push_run_init, fail_run_init, run_init_calls);
    scripted!(combat_result, CombatResult, set_combat_result, push_combat_result, fail_combat_result, combat_result_calls);
    scripted!(combat_frames, CombatFrames, set_combat_frames, push_combat_frames, fail_combat_frames, combat_frames_calls);
    scripted!(pvp_opponent, PvpOpponent, set_pvp_opponent, push_pvp_opponent, fail_pvp_opponent, pvp_opponent_calls);
    scripted!(selected_hero, String, set_selected_hero, push_selected_hero, fail_selected_hero, selected_hero_calls);
    scripted!(player_rank, PlayerRank, set_player_rank, push_player_rank, fail_player_rank, player_rank_calls);
}

impl GameMemory for FakeMemory {
    fn read_board(&mut self) -> MemoryResult<BoardState> {
        self.scripts.lock().unwrap().board.next()
    }

    fn read_run_init(&mut self) -> MemoryResult<RunInit> {
        self.scripts.lock().unwrap().run_init.next()
    }

    fn read_combat_result(&mut self) -> MemoryResult<CombatResult> {
        self.scripts.lock().unwrap().combat_result.next()
    }

    fn read_combat_frames(&mut self) -> MemoryResult<CombatFrames> {
        self.scripts.lock().unwrap().combat_frames.next()
    }

    fn read_pvp_opponent(&mut self) -> MemoryResult<PvpOpponent> {
        self.scripts.lock().unwrap().pvp_opponent.next()
    }

    fn read_selected_hero(&mut self) -> MemoryResult<String> {
        self.scripts.lock().unwrap().selected_hero.next()
    }

    fn read_player_rank(&mut self) -> MemoryResult<PlayerRank> {
        self.scripts.lock().unwrap().player_rank.next()
    }
}
