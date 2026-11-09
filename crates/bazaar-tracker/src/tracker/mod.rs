//! The tracker service: follows `Player.log`, reads the game, runs the run state machine and saves
//! every Event to `history.db`, in the background, reporting what happens over a channel.
//!
//! The service owns its threads and never blocks the caller:
//! - a log thread follows the file and passes its lines on;
//! - a machine thread runs the [`RunMachine`], applies its outputs to the [`Store`] in order, and
//!   retries a failed save (with a growing delay) instead of dropping it;
//! - when started on the live game, a connection thread keeps the game attached.
//!
//! The caller reads [`Notice`]s from [`Tracker::notices`], asks for the run being played with
//! [`Control::pick_up`] (from [`Tracker::control`]), and stops the service with [`Tracker::stop`]
//! (or by dropping it). Attaching to the game picks up its run too, so a tracker started mid-run
//! follows it without waiting for something to happen in the game.

pub mod store;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub use store::Store;

use crate::log::LogFollower;
use crate::memory::{BoardState, ConnectionChange, GameMemory};
use crate::run::{Clock, Event, Output, PickUp, RunMachine, SystemClock};

/// How the service is set up.
#[derive(Clone, Debug)]
pub struct TrackerConfig {
    /// The `Player.log` to follow.
    pub log_path: PathBuf,
    /// How often the log is looked at.
    pub poll_interval: Duration,
    /// How long to wait before the first retry of a failed save; it doubles up to `max_retry_delay`.
    pub retry_delay: Duration,
    pub max_retry_delay: Duration,
    /// How often the game's board is read for [`Notice::LiveSnapshot`] while the game is attached.
    /// `None` turns it off.
    pub live_interval: Option<Duration>,
}

impl TrackerConfig {
    pub fn new(log_path: impl Into<PathBuf>) -> Self {
        Self { log_path: log_path.into(), poll_interval: crate::log::follow::POLL_INTERVAL, retry_delay: Duration::from_secs(1), max_retry_delay: Duration::from_secs(30), live_interval: Some(Duration::from_secs(1)) }
    }
}

/// What the service reports.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant, reason = "Events are almost all of the notices, so boxing them would only add a hop")]
pub enum Notice {
    /// The game was found or lost.
    Connection(ConnectionChange),
    /// An Event was saved.
    EventSaved(Event),
    /// A Run's `local-` id was replaced by its real one, and the history now says so.
    RunRebound { old_run_id: String, new_run_id: String },
    /// Something could not be saved. It will be tried again, in `retry_in`; nothing is dropped.
    SaveFailed { what: String, error: String, retry_in: Duration },
    /// Reading `Player.log` failed. It is tried again on the next pass.
    LogError(String),
    /// What the game shows now, read about once a second while it is attached and sent when it
    /// changes. `None` means the game has no board to read (no run). It is never saved.
    LiveSnapshot(Option<BoardState>),
    /// What picking up the game's run did: asked for with [`Control::pick_up`], or on attaching.
    PickUp(PickUp),
}

enum Input {
    Line(String),
    Connection(ConnectionChange),
    PickUp,
    Stop,
}

/// Asks a running service for things, from any thread. Cheap to clone; a stopped service ignores it.
#[derive(Clone, Debug)]
pub struct Control {
    input: Sender<Input>,
}

impl Control {
    /// Follow the run the game is in now, without waiting for a log line. The service answers with
    /// [`Notice::PickUp`].
    pub fn pick_up(&self) {
        let _ = self.input.send(Input::PickUp);
    }
}

/// A running tracker service.
pub struct Tracker {
    stop: Arc<AtomicBool>,
    input: Sender<Input>,
    notices: Receiver<Notice>,
    threads: Vec<JoinHandle<()>>,
}

impl Tracker {
    /// Start tracking with this game memory, saving to `store`. `connection` carries the game
    /// memory's attach and detach changes, if it has any; a detach resets the run state machine.
    pub fn start(config: TrackerConfig, memory: Box<dyn GameMemory>, store: Box<dyn Store>, clock: Arc<dyn Clock>, connection: Option<Receiver<ConnectionChange>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let (input, inputs) = mpsc::channel();
        let (notices_out, notices) = mpsc::channel();
        let mut threads = Vec::new();

        // The log follower.
        {
            let (stop, input, notices) = (stop.clone(), input.clone(), notices_out.clone());
            let (path, interval) = (config.log_path.clone(), config.poll_interval);
            threads.push(spawn("tracker-log", move || {
                LogFollower::new(path).run_every(
                    interval,
                    &stop,
                    |lines| {
                        for line in lines {
                            if input.send(Input::Line(line)).is_err() {
                                return;
                            }
                        }
                    },
                    |error| {
                        let _ = notices.send(Notice::LogError(error.to_string()));
                    },
                );
            }));
        }

        // Connection changes, passed on to the machine thread in order with the log lines.
        if let Some(connection) = connection {
            let (input, stop) = (input.clone(), stop.clone());
            threads.push(spawn("tracker-connection", move || {
                while !stop.load(Ordering::Relaxed) {
                    match connection.recv_timeout(Duration::from_millis(25)) {
                        Ok(change) => {
                            if input.send(Input::Connection(change)).is_err() {
                                return;
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            }));
        }

        // The run state machine and the store.
        {
            let machine = RunMachine::new(memory, clock.clone());
            threads.push(spawn("tracker-machine", move || run_machine(machine, store, clock, inputs, notices_out, config)));
        }

        Self { stop, input, notices, threads }
    }

    /// Start tracking the live game and save to `history`.
    #[cfg(any(windows, target_os = "macos"))]
    pub fn start_live(config: TrackerConfig, history: Arc<crate::history::History>) -> Self {
        use crate::memory::game::BazaarMemory;
        let memory = BazaarMemory::new();
        let (changes, connection) = mpsc::channel();
        memory.report_changes_to(changes);
        let mut tracker = Self::start(config, Box::new(memory.clone()), Box::new(history), Arc::new(SystemClock), Some(connection));
        let stop = tracker.stop.clone();
        tracker.threads.push(spawn("tracker-game", move || memory.run_connection_loop(&stop)));
        tracker
    }

    /// A handle that asks the service for things.
    pub fn control(&self) -> Control {
        Control { input: self.input.clone() }
    }

    /// What the service reports. Reading never blocks the service.
    pub fn notices(&self) -> &Receiver<Notice> {
        &self.notices
    }

    /// Stop the service and wait for its threads to end.
    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.input.send(Input::Stop);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        self.shut_down();
    }
}

fn spawn(name: &str, body: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    std::thread::Builder::new().name(name.into()).spawn(body).expect("the operating system can start a thread")
}

/// One thing to apply to the store, in the order the machine produced it.
#[allow(clippy::large_enum_variant, reason = "Events are almost all of the work, so boxing them would only add a hop")]
enum Op {
    Save(Event),
    Rebind { old_run_id: String, new_run_id: String },
}

fn run_machine(mut machine: RunMachine, mut store: Box<dyn Store>, clock: Arc<dyn Clock>, inputs: Receiver<Input>, notices: Sender<Notice>, config: TrackerConfig) {
    let mut pending: std::collections::VecDeque<Op> = std::collections::VecDeque::new();
    let mut retry_at: Option<Instant> = None;
    let mut delay = config.retry_delay;
    let tick_every = Duration::from_millis(25);
    // The live board: read from this thread, in turn with the machine's own reads, only while
    // attached, and sent only when it has changed.
    let mut attached = false;
    let mut next_live_ms = 0;
    let mut last_live: Option<Option<BoardState>> = None;

    loop {
        let mut wait = tick_every;
        if let Some(due) = machine.next_due_ms() {
            wait = wait.min(Duration::from_millis((due - clock.now_ms()).max(0) as u64));
        }
        if let Some(at) = retry_at {
            wait = wait.min(at.saturating_duration_since(Instant::now()));
        }
        match inputs.recv_timeout(wait) {
            Ok(Input::Line(line)) => machine.on_raw_line(&line),
            Ok(Input::Connection(change)) => {
                let lost = matches!(change, ConnectionChange::Disconnected { .. });
                attached = !lost;
                next_live_ms = 0;
                last_live = None;
                let _ = notices.send(Notice::Connection(change));
                if lost {
                    machine.reset();
                } else {
                    // A run already being played is followed at once.
                    let _ = notices.send(Notice::PickUp(machine.pick_up()));
                }
            }
            Ok(Input::PickUp) => {
                let result = if attached { machine.pick_up() } else { PickUp::NotAttached };
                let _ = notices.send(Notice::PickUp(result));
            }
            Ok(Input::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => machine.tick(),
        }
        if let (Some(every), true) = (config.live_interval, attached)
            && clock.now_ms() >= next_live_ms
        {
            next_live_ms = clock.now_ms() + every.as_millis() as i64;
            if let Ok(board) = machine.read_live()
                && last_live.as_ref() != Some(&board)
            {
                last_live = Some(board.clone());
                let _ = notices.send(Notice::LiveSnapshot(board));
            }
        }
        for output in machine.take_outputs() {
            pending.push_back(match output {
                Output::Event(event) => Op::Save(event),
                Output::Rebound { old_run_id, new_run_id } => Op::Rebind { old_run_id, new_run_id },
            });
        }
        apply_pending(&mut pending, &mut *store, &notices, &config, &mut retry_at, &mut delay);
    }
}

/// Apply as much of `pending` as can be, in order. A failure stops at that item, reports it, and
/// schedules another try; later items wait behind it so the history keeps the order.
fn apply_pending(pending: &mut std::collections::VecDeque<Op>, store: &mut dyn Store, notices: &Sender<Notice>, config: &TrackerConfig, retry_at: &mut Option<Instant>, delay: &mut Duration) {
    if retry_at.is_some_and(|at| Instant::now() < at) {
        return;
    }
    while let Some(op) = pending.front() {
        let (what, result) = match op {
            Op::Save(event) => (format!("event {} ({})", event.id, event.kind.name()), store.save_event(event)),
            Op::Rebind { old_run_id, new_run_id } => (format!("run {old_run_id} as {new_run_id}"), store.rebind_run(old_run_id, new_run_id)),
        };
        match result {
            Ok(()) => {
                match pending.pop_front() {
                    Some(Op::Save(event)) => {
                        let _ = notices.send(Notice::EventSaved(event));
                    }
                    Some(Op::Rebind { old_run_id, new_run_id }) => {
                        let _ = notices.send(Notice::RunRebound { old_run_id, new_run_id });
                    }
                    None => {}
                }
                *retry_at = None;
                *delay = config.retry_delay;
            }
            Err(error) => {
                let _ = notices.send(Notice::SaveFailed { what, error, retry_in: *delay });
                *retry_at = Some(Instant::now() + *delay);
                *delay = (*delay * 2).min(config.max_retry_delay);
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "tracker_tests.rs"]
mod tests;
