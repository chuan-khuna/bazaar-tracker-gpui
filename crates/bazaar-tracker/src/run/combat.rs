//! Combats: both boards just before a fight, and how it ended.
//!
//! The boards are captured 350 ms after entering a combat state (or after cards spawn before the
//! fight is played); "Combat prepared" says the fight starts and sends CombatStart; leaving the
//! combat state sends CombatEnd. If neither the simulation nor the game's counters say who won, the
//! end is tried again after 1200 ms and then sent whatever it finds.

use super::event::{EventKind, Outcome};
use super::machine::{COMBAT_CAPTURE_DELAY_MS, COMBAT_END_RETRY_MS, Emit, RunMachine, TimerKind, is_combat_state};
use crate::enums::known;
use crate::memory::{BoardState, CombatResult};

/// What is known about the combat in progress.
#[derive(Debug, Default)]
pub(super) struct CombatState {
    pub is_pvp: bool,
    /// The id the CombatStart Event has (or will have), so CombatEnd can refer to it.
    pub start_event_id: Option<String>,
    /// The boards captured shortly after the combat began.
    pub pre_board: Option<BoardState>,
    /// Whether the fight is being played: boards are no longer captured once it is.
    pub playback_started: bool,
    /// The number of frames the game said the simulation has.
    pub expected_frames: Option<i32>,
    pub pre_victories: Option<i32>,
    pub pre_losses: Option<i32>,
    pub pre_day: Option<i32>,
    pub pre_hour: Option<i32>,
}

/// A CombatStart waiting to be built.
#[derive(Clone, Debug)]
pub struct PendingCombatStart {
    pub(super) run_id: String,
    pub(super) event_id: String,
    pub(super) is_pvp: bool,
    pub(super) pre_board: Option<BoardState>,
}

/// A CombatEnd waiting to be built.
#[derive(Clone, Debug)]
pub struct PendingCombatEnd {
    pub(super) run_id: String,
    pub(super) start_event_id: Option<String>,
    pub(super) expected_frames: Option<i32>,
    pub(super) was_pvp: bool,
    /// Set when the game's state already says how it ended (an end-of-run state).
    pub(super) known: Option<Outcome>,
    pub(super) pre_victories: Option<i32>,
    pub(super) pre_losses: Option<i32>,
    pub(super) pre_day: Option<i32>,
    pub(super) pre_hour: Option<i32>,
    /// Whether this is the second try.
    pub(super) retried: bool,
}

/// What the simulation says, once its frame count has been checked against the expected one.
struct Sim {
    result: Option<CombatResult>,
    diag: Option<String>,
}

fn outcome_from_sim(winner: Option<i32>, loser: Option<i32>) -> Option<Outcome> {
    match (winner, loser) {
        (Some(0), Some(1)) => Some(Outcome::Win),
        (Some(1), Some(0)) => Some(Outcome::Loss),
        _ => None,
    }
}

impl RunMachine {
    /// A combat state was entered: forget the last fight and capture the boards shortly.
    pub(super) fn enter_combat(&mut self, state: &str) {
        self.combat.is_pvp |= state == "PVPCombatState";
        self.combat.start_event_id = None;
        self.combat.pre_board = None;
        self.combat.playback_started = false;
        self.schedule_combat_capture();
    }

    fn schedule_combat_capture(&mut self) {
        self.combat_capture_generation += 1;
        let generation = self.combat_capture_generation;
        self.schedule(COMBAT_CAPTURE_DELAY_MS, generation, TimerKind::CombatCapture);
    }

    /// Cards spawned in a combat that is not being played yet: capture the boards again, once they settle.
    pub(super) fn on_cards_spawned(&mut self) {
        if self.app_state.as_deref().is_some_and(is_combat_state) && !self.combat.playback_started {
            self.schedule_combat_capture();
        }
    }

    pub(super) fn on_combat_capture_fired(&mut self, generation: u64) {
        let still_waiting = |machine: &Self| generation == machine.combat_capture_generation && machine.app_state.as_deref().is_some_and(is_combat_state) && !machine.combat.playback_started;
        if !still_waiting(self) {
            return;
        }
        let board = self.read_board();
        // The fight may have started while the board was being read.
        if still_waiting(self) {
            self.combat.pre_board = board;
        }
    }

    /// "Combat prepared": the fight is about to be played.
    pub(super) fn handle_combat_prepared(&mut self, frames: Option<u32>) {
        if let Some(frames) = frames {
            self.combat.expected_frames = i32::try_from(frames).ok();
        }
        let state = self.app_state.as_deref().unwrap_or_default();
        if state.starts_with("EndRun") || (self.run_ended && !is_combat_state(state)) {
            return;
        }
        self.ensure_run();
        let Some(run_id) = self.run_id.clone() else { return };
        // The fight is on: no more settling, and no more capturing.
        self.settle_generation += 1;
        self.combat_capture_generation += 1;
        self.combat.playback_started = true;
        let event_id = (self.new_id)();
        self.combat.start_event_id = Some(event_id.clone());
        if let Some(board) = &self.combat.pre_board {
            let stats = &board.player.stats;
            self.combat.pre_victories = stats.victories;
            self.combat.pre_losses = stats.losses;
            self.combat.pre_day = stats.day;
            self.combat.pre_hour = stats.hour;
        }
        let pending = PendingCombatStart { run_id, event_id, is_pvp: self.combat.is_pvp, pre_board: self.combat.pre_board.clone() };
        self.queue.push_back(Emit::CombatStart(Box::new(pending)));
    }

    pub(super) fn build_combat_start(&mut self, pending: PendingCombatStart) {
        let board = pending.pre_board.or_else(|| self.read_board()).unwrap_or_default();
        let opponent = if pending.is_pvp { self.memory.read_pvp_opponent().ok().flatten().unwrap_or_default() } else { Default::default() };
        let kind = EventKind::CombatStart {
            player: board.player.clone(),
            opponent: board.opponent.clone(),
            is_pvp: pending.is_pvp,
            opponent_name: opponent.name,
            opponent_hero: opponent.hero.or_else(|| board.opponent.stats.hero.clone()),
            opponent_rating: opponent.rating,
            opponent_level: opponent.level,
        };
        self.emit_with_id(pending.event_id, &pending.run_id, board.player.stats.day, board.player.stats.hour, kind);
    }

    /// The game left a combat state: the fight is over. `known` is its outcome when the new state says.
    pub(super) fn queue_combat_end(&mut self, known: Option<Outcome>) {
        let was_pvp = std::mem::take(&mut self.combat.is_pvp);
        let expected_frames = self.combat.expected_frames.take();
        let start_event_id = self.combat.start_event_id.take();
        self.combat.pre_board = None;
        self.combat.playback_started = false;
        // A fight with no CombatStart (the tracker joined late) has nothing to end.
        let (Some(run_id), Some(start_event_id)) = (self.run_id.clone(), start_event_id) else { return };
        let pending = PendingCombatEnd {
            run_id,
            start_event_id: Some(start_event_id),
            expected_frames,
            was_pvp,
            known,
            pre_victories: self.combat.pre_victories,
            pre_losses: self.combat.pre_losses,
            pre_day: self.combat.pre_day,
            pre_hour: self.combat.pre_hour,
            retried: false,
        };
        self.queue.push_back(Emit::CombatEnd(Box::new(pending)));
    }

    /// The simulation's verdict, ignored if its frame count is not the one expected (a stale one).
    fn read_sim(&mut self, expected_frames: Option<i32>) -> Sim {
        let result = self.memory.read_combat_result().ok().flatten();
        match (&result, expected_frames) {
            (Some(found), Some(expected)) if found.frame_count.is_some_and(|frames| frames != expected) => Sim { result, diag: Some("stale (frame mismatch)".to_owned()) },
            _ => Sim { result, diag: None },
        }
    }

    pub(super) fn build_combat_end(&mut self, pending: PendingCombatEnd) {
        let sim = self.read_sim(pending.expected_frames);
        let board = self.read_board();
        let stats = board.as_ref().map(|board| board.player.stats.clone()).unwrap_or_default();
        let opponent_hp = board.as_ref().and_then(|board| board.opponent.stats.health);
        let sim_verdict = if sim.diag.is_none() { sim.result.as_ref().and_then(|result| outcome_from_sim(result.winner, result.loser)) } else { None };

        let outcome = match pending.known {
            Some(known) => known,
            None => self.derive_outcome(sim_verdict, &pending, board.as_ref(), opponent_hp),
        };
        if outcome == Outcome::Unknown && pending.known.is_none() && !pending.retried {
            let mut again = pending;
            again.retried = true;
            self.schedule(COMBAT_END_RETRY_MS, 0, TimerKind::CombatEndRetry(Box::new(again)));
            return;
        }

        // A PvP result may reach the counters after the state changes: they are at least one more.
        let bump = |after: Option<i32>, before: Option<i32>, wanted: bool| match (wanted && pending.was_pvp, before) {
            (true, Some(before)) => Some(after.map_or(before + 1, |after| after.max(before + 1))),
            _ => after,
        };
        let kind = EventKind::CombatEnd {
            outcome,
            combat_start_event_id: pending.start_event_id.clone(),
            player_hp_after: stats.health,
            opponent_hp_after: opponent_hp,
            victories_after: bump(stats.victories, pending.pre_victories, outcome == Outcome::Win),
            losses_after: bump(stats.losses, pending.pre_losses, outcome == Outcome::Loss),
            sim_winner: sim.result.as_ref().and_then(|result| result.winner),
            sim_loser: sim.result.as_ref().and_then(|result| result.loser),
            sim_frames: sim.result.as_ref().and_then(|result| result.frame_count),
            expected_sim_frames: pending.expected_frames,
            sim_diag: sim.diag,
        };
        let day = pending.pre_day.or(stats.day);
        let hour = pending.pre_hour.or(stats.hour);
        self.emit(&pending.run_id, day, hour, kind);
    }

    /// Decide the outcome from the simulation, then the counters, then the run state, then HP.
    fn derive_outcome(&self, sim: Option<Outcome>, pending: &PendingCombatEnd, board: Option<&BoardState>, opponent_hp: Option<i32>) -> Outcome {
        if let Some(outcome) = sim {
            return outcome;
        }
        let stats = board.map(|board| &board.player.stats);
        let grew = |after: Option<i32>, before: Option<i32>| matches!((after, before), (Some(after), Some(before)) if after > before);
        if grew(stats.and_then(|s| s.victories), pending.pre_victories) {
            return Outcome::Win;
        }
        if grew(stats.and_then(|s| s.losses), pending.pre_losses) {
            return Outcome::Loss;
        }
        let run_state = board.and_then(|board| board.run_state.as_deref()).and_then(crate::enums::run_state::value_of);
        match run_state {
            Some(known::RUN_STATE_END_RUN_DEFEAT) => return Outcome::Loss,
            Some(known::RUN_STATE_END_RUN_VICTORY) => return Outcome::Win,
            _ => {}
        }
        if opponent_hp.is_some_and(|hp| hp <= 0) {
            return Outcome::Win;
        }
        if stats.and_then(|s| s.health).is_some_and(|hp| hp <= 0) {
            return Outcome::Loss;
        }
        Outcome::Unknown
    }

    /// The second try at a combat end that could not be decided.
    pub(super) fn on_combat_end_retry(&mut self, pending: PendingCombatEnd) {
        self.queue.push_back(Emit::CombatEnd(Box::new(pending)));
    }
}

#[cfg(test)]
#[path = "combat_tests.rs"]
mod tests;
