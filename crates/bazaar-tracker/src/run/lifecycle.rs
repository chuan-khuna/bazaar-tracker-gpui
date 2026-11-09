//! Where a Run starts, ends and changes identity.

use super::event::{EventKind, Outcome};
use super::machine::{Emit, LastStats, Output, RunMachine, TimerKind};
use crate::memory::RunInit;

/// What [`RunMachine::pick_up`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickUp {
    /// It follows this run now; its RunStarted is on its way.
    Following(String),
    /// A run is followed already.
    AlreadyFollowing(String),
    /// The game has no board: no run is being played.
    NoRun,
    /// The game's run is over (or is one seen ending).
    RunOver,
    /// Memory doesn't name the run yet.
    NoRunId,
    /// The game isn't attached.
    NotAttached,
}

impl PickUp {
    /// What happened, in a few words: `Following run 3f2a…`.
    pub fn describe(&self) -> String {
        match self {
            PickUp::Following(run_id) => format!("Following run {run_id}"),
            PickUp::AlreadyFollowing(run_id) => format!("Already following run {run_id}"),
            PickUp::NoRun => "No run is being played".to_owned(),
            PickUp::RunOver => "The game's run is over".to_owned(),
            PickUp::NoRunId => "The game doesn't name its run yet; try again in a moment".to_owned(),
            PickUp::NotAttached => "The game isn't attached".to_owned(),
        }
    }
}

/// A run end that is waiting to be built: what was true when the run ended.
#[derive(Clone, Debug)]
pub struct PendingRunEnd {
    pub(super) run_id: String,
    pub(super) outcome: Outcome,
    pub(super) ranked: bool,
    pub(super) stats: LastStats,
    pub(super) pre_run_rating: Option<i32>,
    /// How long the rating has been waited for, for a ranked run.
    pub(super) waited_ms: i64,
}

impl RunMachine {
    /// Begin following a new Run. With no real id yet, it gets a `local-` one.
    pub(super) fn start_run(&mut self, info: Option<RunInit>) {
        let run_id = info.as_ref().and_then(|info| info.run_id.clone()).unwrap_or_else(|| format!("local-{}", (self.new_id)().replace('-', "")));
        self.run_id = Some(run_id.clone());
        self.run_ended = false;
        self.run_has_activity = false;
        self.run_play_mode = info.as_ref().and_then(|info| info.play_mode.clone());
        self.pre_run_rating = None;
        self.last_stats = LastStats::default();
        self.last_player_item_ids.clear();
        self.sold_ids.clear();
        self.selected_ids.clear();
        self.reported_option_ids.clear();
        self.option_event_of.clear();
        self.dealt_current.clear();
        self.dealt_previous.clear();
        self.options_dirty = false;
        self.entity_index.clear();
        self.queue.push_back(Emit::RunStarted { run_id, info, attempts: 0 });
    }

    /// Make sure there is a Run to put an activity Event in. It never starts a new Run once one is
    /// set, even an ended one: that is for a run-init that names a different run.
    pub(super) fn ensure_run(&mut self) {
        if self.run_id.is_none() {
            let mut info = self.pending_run_init.clone();
            if info.is_none() && self.read_board().is_some() {
                info = self.read_run_init();
            }
            // A run id that has already finished must not be started again: use a fresh local one.
            if info.as_ref().and_then(|info| info.run_id.as_ref()).is_some_and(|id| self.ended_run_ids.contains(id)) {
                info = None;
            }
            self.start_run(info);
        }
        self.run_has_activity = true;
    }

    /// Follow the run the game is in now, without waiting for a log line: as when the tracker
    /// starts, or the game is attached, while a run is being played and nothing has happened in it
    /// yet. Only a run memory names by its real id is picked up, so it continues that Run in the
    /// history (its RunStarted clears an end the history may have, so a run that is over, or that
    /// this machine saw end, is left alone). A picked-up run counts as having activity: it was
    /// being played before. After the followed run has ended, a new run is picked up too.
    pub fn pick_up(&mut self) -> PickUp {
        if let Some(run_id) = self.run_id.as_ref().filter(|_| !self.run_ended) {
            return PickUp::AlreadyFollowing(run_id.clone());
        }
        let Some(board) = self.read_board() else { return PickUp::NoRun };
        if matches!(board.run_state.as_deref(), Some("EndRunVictory" | "EndRunDefeat" | "NewRun" | "Shutdown")) {
            return PickUp::RunOver;
        }
        let info = self.read_run_init();
        let Some(run_id) = info.as_ref().and_then(|info| info.run_id.clone()) else { return PickUp::NoRunId };
        if self.ended_run_ids.contains(&run_id) || self.run_id.as_deref() == Some(run_id.as_str()) {
            return PickUp::RunOver;
        }
        self.start_run(info);
        self.run_has_activity = true;
        self.pump();
        PickUp::Following(run_id)
    }

    /// The game's run-init line: either memory already tells us which run this is, or it will.
    pub(super) fn handle_run_initialized(&mut self) {
        match self.pending_run_init.clone() {
            Some(info) => self.resolve_run_init(info),
            None => self.run_boundary_pending = true,
        }
    }

    /// The game says the run is `info`. If we do not follow that run yet, either learn the real id
    /// of the run we are following, or end it and begin the new one.
    pub(super) fn resolve_run_init(&mut self, info: RunInit) {
        let Some(new_id) = info.run_id.clone() else {
            if self.run_id.is_none() {
                self.start_run(Some(info));
            }
            return;
        };
        if self.run_id.as_deref() == Some(new_id.as_str()) {
            return;
        }
        if self.run_is_local() && !self.run_ended {
            self.rebind_run(new_id, info.play_mode);
        } else {
            self.end_run(Outcome::Unknown);
            self.start_run(Some(info));
        }
    }

    /// The run we follow under a `local-` id has a real id after all.
    fn rebind_run(&mut self, new_id: String, play_mode: Option<String>) {
        let Some(old_id) = self.run_id.replace(new_id.clone()) else { return };
        if play_mode.is_some() {
            self.run_play_mode = play_mode;
        }
        self.outputs.push(Output::Rebound { old_run_id: old_id, new_run_id: new_id });
    }

    /// The run is over. Nothing happens unless there is a run that has not ended and has had some
    /// activity.
    pub(super) fn end_run(&mut self, outcome: Outcome) {
        let Some(run_id) = self.run_id.clone() else { return };
        if self.run_ended || !self.run_has_activity {
            return;
        }
        self.run_ended = true;
        self.ended_run_ids.insert(run_id.clone());
        let pending = PendingRunEnd { run_id, outcome, ranked: self.is_ranked(), stats: self.last_stats.clone(), pre_run_rating: self.pre_run_rating, waited_ms: 0 };
        self.queue.push_back(Emit::RunEnded(Box::new(pending)));
    }

    /// Build the RunStarted Event, or say (by returning the work again) that the hero is not known
    /// yet and another look should be taken shortly.
    pub(super) fn build_run_started(&mut self, run_id: &str, info: Option<RunInit>, attempts: u32) -> Option<Emit> {
        let board = self.read_board();
        let hero = board.as_ref().and_then(|board| board.player.stats.hero.clone()).or_else(|| self.memory.read_selected_hero().ok().flatten());
        if hero.is_none() && attempts < super::machine::HERO_POLLS {
            return Some(Emit::RunStarted { run_id: run_id.to_owned(), info, attempts: attempts + 1 });
        }
        let rank = self.memory.read_player_rank().ok().flatten();
        if self.run_id.as_deref() == Some(run_id) {
            self.pre_run_rating = rank.as_ref().map(|rank| rank.rating);
        }
        let info = info.unwrap_or_default();
        let (day, hour) = board.as_ref().map_or((None, None), |board| (board.player.stats.day, board.player.stats.hour));
        let kind = EventKind::RunStarted {
            play_mode: info.play_mode,
            build_id: info.build_id,
            environment: info.environment,
            hero,
            rating: rank.as_ref().map(|rank| rank.rating),
            rank: rank.as_ref().map(|rank| rank.rank.clone()),
            rank_division: rank.as_ref().map(|rank| rank.division),
        };
        self.emit(run_id, day, hour, kind);
        None
    }

    /// Build the RunEnded Event from the board and rank as they are now.
    pub(super) fn build_run_ended(&mut self, pending: PendingRunEnd) {
        let rank = self.memory.read_player_rank().ok().flatten();
        // A ranked run end waits (up to 4 seconds) for the rating to change before it is sent.
        let rating_unchanged = pending.pre_run_rating.is_some() && rank.as_ref().map(|rank| rank.rating) == pending.pre_run_rating;
        let rating_missing = pending.pre_run_rating.is_some() && rank.is_none();
        if pending.ranked && (rating_unchanged || rating_missing) && pending.waited_ms < super::machine::RANK_SETTLE_MS {
            let mut again = pending;
            again.waited_ms += super::machine::RANK_POLL_MS;
            self.schedule(super::machine::RANK_POLL_MS, 0, TimerKind::RankSettle(Box::new(again)));
            return;
        }
        let board = self.read_board();
        let stats = board.as_ref().map(|board| board.player.stats.clone()).unwrap_or_default();
        let day = stats.day.or(pending.stats.day);
        let hour = stats.hour.or(pending.stats.hour);
        let kind = EventKind::RunEnded {
            outcome: pending.outcome,
            victories: stats.victories.or(pending.stats.victories),
            losses: stats.losses.or(pending.stats.losses),
            gold: stats.gold,
            health_max: stats.health_max,
            prestige: stats.prestige,
            level: stats.level,
            income: stats.income,
            rating_after: rank.as_ref().map(|rank| rank.rating),
            rank_after: rank.as_ref().map(|rank| rank.rank.clone()),
            rank_division_after: rank.as_ref().map(|rank| rank.division),
        };
        self.emit(&pending.run_id, day, hour, kind);
    }

    /// Time to look at the rating again for a ranked run end.
    pub(super) fn on_rank_settle(&mut self, pending: PendingRunEnd) {
        self.queue.push_back(Emit::RunEnded(Box::new(pending)));
    }
}
