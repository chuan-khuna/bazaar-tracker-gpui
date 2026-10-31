//! The run state machine. Log lines are triggers and keys; game memory gives the data. An Event is
//! built when it is sent, so its Snapshot reflects that moment, not the moment the line arrived.
//!
//! The machine has no threads and no timers of its own. It is told about each log line
//! ([`RunMachine::on_line`]) and about the passing of time ([`RunMachine::tick`]), and hands back
//! what it produced ([`RunMachine::take_outputs`]). Delays (500 ms to settle the options, 350 ms
//! before the combat boards are captured, and so on) are timers on the machine's [`Clock`], each
//! cancelled by a generation counter when a newer line supersedes it, so a test drives them with a
//! [`super::clock::ManualClock`].

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use super::clock::Clock;
use super::event::{Event, EventKind, Outcome, is_local_run_id};
use crate::log::LogLine;
use crate::memory::{BoardState, GameMemory, Item, RunInit};

/// How long the options take to settle after the game deals them.
pub const SETTLE_DELAY_MS: i64 = 500;
/// How long after a combat begins the two boards are captured.
pub const COMBAT_CAPTURE_DELAY_MS: i64 = 350;
/// How long to wait before trying again to decide how a combat ended.
pub const COMBAT_END_RETRY_MS: i64 = 1200;
/// How long a ranked run end waits for the rating to change.
pub const RANK_SETTLE_MS: i64 = 4000;
/// How often the rating is looked at while waiting for it.
pub const RANK_POLL_MS: i64 = 500;
/// How long between looks for the hero when a run starts, and how many looks (3 seconds).
pub const HERO_POLL_MS: i64 = 500;
pub const HERO_POLLS: u32 = 6;
/// How many retired option ids are remembered.
pub const DEALT_PREVIOUS_CAP: usize = 40;

/// What the machine produces.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant, reason = "Events are almost all of the outputs, so boxing them would only add a hop")]
pub enum Output {
    /// An Event to save.
    Event(Event),
    /// A Run that began under a `local-` id turned out to have this real id: its Events should move.
    Rebound { old_run_id: String, new_run_id: String },
}

/// The states in which the game offers a choice.
pub fn is_option_state(state: &str) -> bool {
    matches!(state, "ChoiceState" | "EncounterState" | "LootState" | "LevelUpState" | "PedestalState")
}

/// The states in which a combat is played.
pub fn is_combat_state(state: &str) -> bool {
    matches!(state, "CombatState" | "PVPCombatState")
}

/// A delayed action. It fires only if its generation is still the current one.
#[derive(Clone, Debug)]
pub(super) enum TimerKind {
    /// Emit the options that have settled.
    Settle,
    /// Capture both boards for the combat that just began.
    CombatCapture,
    /// Try again to decide how the last combat ended.
    CombatEndRetry(Box<super::combat::PendingCombatEnd>),
    /// A ranked run end has waited long enough for the rating.
    RankSettle(Box<super::lifecycle::PendingRunEnd>),
}

#[derive(Clone, Debug)]
pub(super) struct Timer {
    pub due_ms: i64,
    pub generation: u64,
    pub kind: TimerKind,
}

/// Work waiting to be turned into an Event. Each is built when it reaches the front of the queue,
/// which is when its memory reads happen.
pub(super) enum Emit {
    RunStarted { run_id: String, info: Option<RunInit>, attempts: u32 },
    Options { settle: bool, presented: Option<Vec<String>>, context: String },
    Selected(Box<super::options::Selection>),
    Merge { run_id: String, candidates: Vec<String>, merged_into: String },
    Sold { run_id: String, before: Vec<String> },
    Rerolled { run_id: String },
    RunEnded(Box<super::lifecycle::PendingRunEnd>),
    CombatStart(Box<super::combat::PendingCombatStart>),
    CombatEnd(Box<super::combat::PendingCombatEnd>),
}

/// The numbers from the most recent successful board read that are kept for filling in later.
#[derive(Clone, Debug, Default)]
pub(super) struct LastStats {
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub victories: Option<i32>,
    pub losses: Option<i32>,
}

pub struct RunMachine {
    pub(super) memory: Box<dyn GameMemory>,
    pub(super) clock: Arc<dyn Clock>,
    pub(super) new_id: Box<dyn FnMut() -> String + Send>,

    pub(super) app_state: Option<String>,
    pub(super) run_id: Option<String>,
    pub(super) run_ended: bool,
    pub(super) run_has_activity: bool,
    pub(super) run_play_mode: Option<String>,
    pub(super) run_boundary_pending: bool,
    pub(super) pending_run_init: Option<RunInit>,
    pub(super) ended_run_ids: HashSet<String>,
    pub(super) last_player_item_ids: Vec<String>,
    pub(super) last_stats: LastStats,
    pub(super) pre_run_rating: Option<i32>,

    pub(super) dealt_current: Vec<String>,
    pub(super) dealt_previous: Vec<String>,
    pub(super) reported_option_ids: HashSet<String>,
    pub(super) option_event_of: HashMap<String, String>,
    pub(super) options_dirty: bool,
    pub(super) selected_ids: HashSet<String>,
    pub(super) sold_ids: HashSet<String>,
    pub(super) entity_index: HashMap<String, Item>,

    pub(super) settle_generation: u64,
    pub(super) combat_capture_generation: u64,
    pub(super) combat: super::combat::CombatState,

    pub(super) timers: Vec<Timer>,
    pub(super) queue: VecDeque<Emit>,
    pub(super) blocked_until_ms: Option<i64>,
    pub(super) outputs: Vec<Output>,
}

impl RunMachine {
    /// A machine that reads `memory`, takes its time from `clock` and numbers its Events and local
    /// runs with random ids.
    pub fn new(memory: Box<dyn GameMemory>, clock: Arc<dyn Clock>) -> Self {
        Self::with_ids(memory, clock, Box::new(random_id))
    }

    /// As [`Self::new`], with a chosen source of ids (tests use a counter).
    pub fn with_ids(memory: Box<dyn GameMemory>, clock: Arc<dyn Clock>, new_id: Box<dyn FnMut() -> String + Send>) -> Self {
        Self {
            memory,
            clock,
            new_id,
            app_state: None,
            run_id: None,
            run_ended: false,
            run_has_activity: false,
            run_play_mode: None,
            run_boundary_pending: false,
            pending_run_init: None,
            ended_run_ids: HashSet::new(),
            last_player_item_ids: Vec::new(),
            last_stats: LastStats::default(),
            pre_run_rating: None,
            dealt_current: Vec::new(),
            dealt_previous: Vec::new(),
            reported_option_ids: HashSet::new(),
            option_event_of: HashMap::new(),
            options_dirty: false,
            selected_ids: HashSet::new(),
            sold_ids: HashSet::new(),
            entity_index: HashMap::new(),
            settle_generation: 0,
            combat_capture_generation: 0,
            combat: super::combat::CombatState::default(),
            timers: Vec::new(),
            queue: VecDeque::new(),
            blocked_until_ms: None,
            outputs: Vec::new(),
        }
    }

    /// The id of the Run being followed, if there is one.
    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
    }

    /// Whether the followed Run has ended.
    pub fn run_has_ended(&self) -> bool {
        self.run_ended
    }

    /// Read what the game shows now, for the live view. This is the machine's own memory, so it
    /// goes through the same lock as its reads, one at a time; nothing is remembered from it.
    pub fn read_live(&mut self) -> crate::memory::MemoryResult<BoardState> {
        self.memory.read_board()
    }

    /// Forget the run in progress and everything about it, as when the game is lost. Timers that
    /// were pending are dropped.
    pub fn reset(&mut self) {
        let fresh = Self::with_ids(
            std::mem::replace(&mut self.memory, Box::new(crate::memory::fake::FakeMemory::new())),
            self.clock.clone(),
            std::mem::replace(&mut self.new_id, Box::new(random_id)),
        );
        let outputs = std::mem::take(&mut self.outputs);
        *self = fresh;
        self.outputs = outputs;
    }

    /// Everything produced since the last call, in order.
    pub fn take_outputs(&mut self) -> Vec<Output> {
        std::mem::take(&mut self.outputs)
    }

    /// When the machine next has something to do without a new log line, if it does.
    pub fn next_due_ms(&self) -> Option<i64> {
        self.timers.iter().map(|timer| timer.due_ms).chain(self.blocked_until_ms).min()
    }

    /// Parse and handle one raw line of `Player.log`.
    pub fn on_raw_line(&mut self, raw: &str) {
        if let Some(line) = crate::log::parse_line(raw) {
            self.on_line(&line);
        }
        self.tick();
    }

    /// Handle one parsed log line.
    pub fn on_line(&mut self, line: &LogLine) {
        self.fire_due_timers();
        self.pending_run_init = None;
        if matches!(line, LogLine::RunInitialized) {
            self.pending_run_init = self.read_run_init();
        } else if self.run_id.is_none() || self.run_boundary_pending || self.run_is_local() {
            // While the run's real id is unknown every line tries to learn it.
            if self.read_board().is_some() {
                self.pending_run_init = self.read_run_init();
            }
        }
        if let Some(info) = self.pending_run_init.clone() {
            if self.run_boundary_pending {
                self.run_boundary_pending = false;
                self.resolve_run_init(info);
            } else if self.run_is_local() && !self.run_ended {
                self.resolve_run_init(info);
            }
        }
        self.dispatch(line);
        self.pending_run_init = None;
        self.pump();
    }

    /// Let time pass: fire the timers that are due and emit what was waiting on them.
    pub fn tick(&mut self) {
        self.fire_due_timers();
        self.pump();
    }

    fn dispatch(&mut self, line: &LogLine) {
        match line {
            LogLine::StateChanged { to, .. } => self.handle_state_changed(to),
            LogLine::GameSimSummary { dealt, spawned, .. } => self.handle_game_sim_summary(*dealt, *spawned),
            LogLine::RunInitialized => self.handle_run_initialized(),
            LogLine::CombatPrepared { frames } => self.handle_combat_prepared(*frames),
            LogLine::CardPlaced { instance_id, template_id, target, section } => self.handle_placed(instance_id, template_id, target, section),
            LogLine::CommandCompleted { command, succeeded } => self.handle_command(command, *succeeded),
            LogLine::SkillSelected { skill_id, socket } => self.handle_skill_selected(skill_id, socket),
            LogLine::CombatCompleted => {}
        }
    }

    // --- timers ---------------------------------------------------------------------------

    /// Start a timer of `kind` that fires after `delay_ms` if `generation` is still current.
    pub(super) fn schedule(&mut self, delay_ms: i64, generation: u64, kind: TimerKind) {
        self.timers.push(Timer { due_ms: self.clock.now_ms() + delay_ms, generation, kind });
    }

    pub(super) fn schedule_settle(&mut self) {
        self.settle_generation += 1;
        let generation = self.settle_generation;
        self.schedule(SETTLE_DELAY_MS, generation, TimerKind::Settle);
    }

    fn fire_due_timers(&mut self) {
        loop {
            let now = self.clock.now_ms();
            let Some(index) = self.timers.iter().enumerate().filter(|(_, timer)| timer.due_ms <= now).min_by_key(|(_, timer)| timer.due_ms).map(|(index, _)| index) else {
                return;
            };
            let timer = self.timers.remove(index);
            match timer.kind {
                TimerKind::Settle => {
                    if timer.generation == self.settle_generation && self.app_state.as_deref().is_some_and(is_option_state) {
                        let context = self.app_state.clone().unwrap_or_default();
                        self.queue.push_back(Emit::Options { settle: true, presented: None, context });
                    }
                }
                TimerKind::CombatCapture => self.on_combat_capture_fired(timer.generation),
                TimerKind::CombatEndRetry(pending) => self.on_combat_end_retry(*pending),
                TimerKind::RankSettle(pending) => self.on_rank_settle(*pending),
            }
            self.pump();
        }
    }

    // --- the emit queue ---------------------------------------------------------------------

    /// Build and output what is queued, in order. A run start that is still waiting for the hero
    /// holds up everything behind it for as long as the hero poll lasts.
    pub(super) fn pump(&mut self) {
        while let Some(front) = self.queue.pop_front() {
            if let Some(until) = self.blocked_until_ms {
                if self.clock.now_ms() < until {
                    self.queue.push_front(front);
                    return;
                }
                self.blocked_until_ms = None;
            }
            match front {
                Emit::RunStarted { run_id, info, attempts } => {
                    if let Some(retry) = self.build_run_started(&run_id, info, attempts) {
                        self.blocked_until_ms = Some(self.clock.now_ms() + HERO_POLL_MS);
                        self.queue.push_front(retry);
                        return;
                    }
                }
                Emit::Options { settle, presented, context } => self.build_options(settle, presented, &context),
                Emit::Selected(selection) => self.build_selected(*selection),
                Emit::Merge { run_id, candidates, merged_into } => self.build_merge(&run_id, &candidates, &merged_into),
                Emit::Sold { run_id, before } => self.build_sold(&run_id, &before),
                Emit::Rerolled { run_id } => self.emit(&run_id, None, None, EventKind::Rerolled),
                Emit::RunEnded(pending) => self.build_run_ended(*pending),
                Emit::CombatStart(pending) => self.build_combat_start(*pending),
                Emit::CombatEnd(pending) => self.build_combat_end(*pending),
            }
        }
    }

    // --- output -----------------------------------------------------------------------------

    /// Make an Event for `run_id` and hand it out. A missing day or hour is filled in from the
    /// last board read that had one.
    pub(super) fn emit(&mut self, run_id: &str, day: Option<i32>, hour: Option<i32>, kind: EventKind) {
        let event = Event {
            id: (self.new_id)(),
            run_id: run_id.to_owned(),
            timestamp_ms: self.clock.now_ms(),
            day: day.or(self.last_stats.day),
            hour: hour.or(self.last_stats.hour),
            kind,
        };
        self.outputs.push(Output::Event(event));
    }

    /// As [`Self::emit`] but with a given id, for Events other Events refer to.
    pub(super) fn emit_with_id(&mut self, id: String, run_id: &str, day: Option<i32>, hour: Option<i32>, kind: EventKind) {
        self.emit(run_id, day, hour, kind);
        if let Some(Output::Event(event)) = self.outputs.last_mut() {
            event.id = id;
        }
    }

    /// The id of the Event emitted last.
    pub(super) fn last_event_id(&self) -> Option<String> {
        self.outputs.iter().rev().find_map(|output| match output {
            Output::Event(event) => Some(event.id.clone()),
            Output::Rebound { .. } => None,
        })
    }

    // --- memory ----------------------------------------------------------------------------

    /// A board read, with what is learned from it kept: every card seen, the player's items and the
    /// day, hour and record. A failed read is `None`.
    pub(super) fn read_board(&mut self) -> Option<BoardState> {
        let board = match self.memory.read_board() {
            Ok(Some(board)) => board,
            Ok(None) => return None,
            Err(error) => {
                tracing::debug!("board read failed: {error}");
                return None;
            }
        };
        for item in board.entities.iter().chain(&board.player.board).chain(&board.player.stash).chain(&board.player.skills) {
            self.entity_index.insert(item.instance_id.clone(), item.clone());
        }
        self.last_player_item_ids = board.player_item_ids();
        let stats = &board.player.stats;
        for (slot, value) in [
            (&mut self.last_stats.day, stats.day),
            (&mut self.last_stats.hour, stats.hour),
            (&mut self.last_stats.victories, stats.victories),
            (&mut self.last_stats.losses, stats.losses),
        ] {
            if value.is_some() {
                *slot = value;
            }
        }
        Some(board)
    }

    pub(super) fn read_run_init(&mut self) -> Option<RunInit> {
        match self.memory.read_run_init() {
            Ok(info) => info,
            Err(error) => {
                tracing::debug!("run init read failed: {error}");
                None
            }
        }
    }

    pub(super) fn run_is_local(&self) -> bool {
        self.run_id.as_deref().is_some_and(is_local_run_id)
    }

    pub(super) fn is_ranked(&self) -> bool {
        self.run_play_mode.as_deref().is_some_and(|mode| mode.eq_ignore_ascii_case("Ranked"))
    }

    pub(super) fn outcome_for_state(state: &str) -> Option<Outcome> {
        match state {
            "EndRunVictoryState" => Some(Outcome::Win),
            "EndRunDefeatState" => Some(Outcome::Loss),
            _ => None,
        }
    }
}

/// A fresh random id: 32 hex digits from the clock, a counter and the standard library's hasher
/// (which is randomly seeded per process).
pub fn random_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = std::collections::hash_map::RandomState::new();
    let mut high = state.build_hasher();
    high.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    high.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    let mut low = state.build_hasher();
    low.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    format!("{:016x}{:016x}", high.finish(), low.finish())
}

#[cfg(test)]
#[path = "machine_tests.rs"]
mod tests;
