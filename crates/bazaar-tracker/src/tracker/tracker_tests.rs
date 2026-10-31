use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use super::*;
use crate::history::History;
use crate::memory::fake::FakeMemory;
use crate::memory::{BoardState, PlayerStats, RunInit, Snapshot};
use crate::run::{Event, ManualClock};

const WAIT: Duration = Duration::from_secs(10);

fn config(log: &Path) -> TrackerConfig {
    TrackerConfig { log_path: log.to_path_buf(), poll_interval: Duration::from_millis(15), retry_delay: Duration::from_millis(10), max_retry_delay: Duration::from_millis(40), live_interval: None }
}

fn append(path: &Path, text: &str) {
    let mut file = OpenOptions::new().append(true).create(true).open(path).unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

fn board() -> BoardState {
    BoardState {
        player: Snapshot { stats: PlayerStats { hero: Some("Jules".into()), gold: Some(10), day: Some(1), hour: Some(0), victories: Some(0), losses: Some(0), ..PlayerStats::default() }, ..Snapshot::default() },
        ..BoardState::default()
    }
}

fn memory_with_run(run_id: &str) -> FakeMemory {
    let memory = FakeMemory::new();
    memory.set_board(board());
    memory.set_run_init(RunInit { run_id: Some(run_id.to_owned()), play_mode: Some("Normal".into()), ..RunInit::default() });
    memory
}

/// Read notices until `done` says the ones so far are enough, or ten seconds pass.
fn collect_until(tracker: &Tracker, done: impl Fn(&[Notice]) -> bool) -> Vec<Notice> {
    let started = Instant::now();
    let mut seen = Vec::new();
    while started.elapsed() < WAIT {
        if let Ok(notice) = tracker.notices().recv_timeout(Duration::from_millis(20)) {
            seen.push(notice);
            if done(&seen) {
                return seen;
            }
        }
    }
    panic!("gave up waiting; saw {seen:#?}");
}

fn saved(notices: &[Notice]) -> Vec<&Event> {
    notices
        .iter()
        .filter_map(|notice| match notice {
            Notice::EventSaved(event) => Some(event),
            _ => None,
        })
        .collect()
}

fn kinds(notices: &[Notice]) -> Vec<&'static str> {
    saved(notices).iter().map(|event| event.kind.name()).collect()
}

/// A store that fails its first `failures` calls and remembers what it saved.
struct Flaky {
    failures: usize,
    calls: usize,
    saved: Arc<Mutex<Vec<String>>>,
}

impl Store for Flaky {
    fn save_event(&mut self, event: &Event) -> Result<(), String> {
        self.calls += 1;
        if self.calls <= self.failures {
            return Err(format!("disk is full (try {})", self.calls));
        }
        self.saved.lock().unwrap().push(event.kind.name().to_owned());
        Ok(())
    }

    fn rebind_run(&mut self, _old: &str, _new: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn it_starts_and_stops_cleanly_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let tracker = Tracker::start(
        TrackerConfig::new(dir.path().join("Player.log")),
        Box::new(FakeMemory::new()),
        Box::new(Flaky { failures: 0, calls: 0, saved: Arc::default() }),
        Arc::new(SystemClock),
        None,
    );
    assert!(started.elapsed() < Duration::from_secs(1), "starting does not wait");
    tracker.stop();
    assert!(started.elapsed() < Duration::from_secs(3), "stopping is prompt");
}

#[test]
fn dropping_the_tracker_stops_it_too() {
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    drop(Tracker::start(config(&dir.path().join("Player.log")), Box::new(FakeMemory::new()), Box::new(Flaky { failures: 0, calls: 0, saved: Arc::default() }), Arc::new(SystemClock), None));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_recorded_log_and_scripted_memory_end_as_rows_in_history_db() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Player.log");
    append(&log, "an old line from before the tracker started\n");
    let history = Arc::new(History::open(&dir.path().join("history.db")).unwrap());

    let memory = memory_with_run("run-real-1");
    memory.set_combat_result(crate::memory::CombatResult { winner: Some(0), loser: Some(1), frame_count: Some(659) });
    let tracker = Tracker::start(config(&log), Box::new(memory), Box::new(history.clone()), Arc::new(SystemClock), None);
    // Let the follower take its place at the end of the old file.
    std::thread::sleep(Duration::from_millis(100));

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/logs/real-run-start.log");
    append(&log, &std::fs::read_to_string(fixture).unwrap());

    let mut notices = collect_until(&tracker, |seen| kinds(seen).contains(&"CombatEnd"));
    // The rest of the log is still being processed: take what comes until things go quiet.
    while let Ok(notice) = tracker.notices().recv_timeout(Duration::from_millis(400)) {
        notices.push(notice);
    }
    tracker.stop();

    let notified: Vec<Event> = saved(&notices).into_iter().cloned().collect();
    assert_eq!(notified[0].kind.name(), "RunStarted");
    assert!(notified.iter().any(|event| event.kind.name() == "Selected"));
    assert!(notified.iter().any(|event| event.kind.name() == "CombatStart"));
    assert!(notified.iter().all(|event| event.run_id == "run-real-1"));

    // What was reported is exactly what is in the database, in the same order.
    let runs = history.list_runs().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, "run-real-1");
    assert_eq!(runs[0].hero.as_deref(), Some("Jules"));
    let rows = history.list_events("run-real-1").unwrap();
    let mut expected = notified.clone();
    expected.sort_by_key(|event| event.timestamp_ms);
    assert_eq!(rows.iter().map(|event| &event.id).collect::<Vec<_>>(), expected.iter().map(|event| &event.id).collect::<Vec<_>>());
    assert_eq!(rows, expected);
}

#[test]
fn a_failed_save_is_reported_retried_and_keeps_the_order() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Player.log");
    append(&log, "old\n");
    let store_saved = Arc::new(Mutex::new(Vec::new()));
    let store = Flaky { failures: 2, calls: 0, saved: store_saved.clone() };
    let tracker = Tracker::start(config(&log), Box::new(memory_with_run("run-1")), Box::new(store), Arc::new(SystemClock), None);
    std::thread::sleep(Duration::from_millis(100));

    append(&log, "[00:00:00.000] [HttpGameClient] Command completed: type=RerollCommand, requestId=1, status=200 OK, durationMs=5, result=success, endpoint=/commands\n");
    let notices = collect_until(&tracker, |seen| kinds(seen).len() == 2);
    tracker.stop();

    let failures: Vec<_> = notices.iter().filter(|notice| matches!(notice, Notice::SaveFailed { .. })).collect();
    assert_eq!(failures.len(), 2, "each failure is reported: {notices:#?}");
    let Notice::SaveFailed { what, error, retry_in } = failures[0] else { unreachable!() };
    assert!(what.contains("RunStarted") && error.contains("disk is full"), "{what}: {error}");
    assert_eq!(*retry_in, Duration::from_millis(10));
    let Notice::SaveFailed { retry_in: second, .. } = failures[1] else { unreachable!() };
    assert_eq!(*second, Duration::from_millis(20), "the delay grows");
    // Nothing was dropped or reordered: the RunStarted that failed is still first.
    assert_eq!(*store_saved.lock().unwrap(), ["RunStarted", "Rerolled"]);
}

#[test]
fn connection_changes_are_reported_and_losing_the_game_resets_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Player.log");
    append(&log, "old\n");
    let (changes, connection) = mpsc::channel();
    let saved_events = Arc::new(Mutex::new(Vec::new()));
    let store = Flaky { failures: 0, calls: 0, saved: saved_events.clone() };
    let tracker = Tracker::start(config(&log), Box::new(memory_with_run("run-1")), Box::new(store), Arc::new(SystemClock), Some(connection));
    std::thread::sleep(Duration::from_millis(100));

    changes.send(ConnectionChange::Connected { pid: 42, unity_version: "test".into() }).unwrap();
    let run_initialized = "[00:00:00.000] [GameMessageHandler] Handled: type=NetMessageRunInitialized, id=Ydf, durationMs=0, result=success\n";
    append(&log, run_initialized);
    let notices = collect_until(&tracker, |seen| kinds(seen).contains(&"RunStarted"));
    assert!(notices.contains(&Notice::Connection(ConnectionChange::Connected { pid: 42, unity_version: "test".into() })));

    // The same run id again changes nothing...
    append(&log, run_initialized);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(saved_events.lock().unwrap().len(), 1);

    // ...but after the game is lost and found again, the run is followed afresh.
    changes.send(ConnectionChange::Disconnected { reason: "the game process exited".into() }).unwrap();
    collect_until(&tracker, |seen| seen.iter().any(|notice| matches!(notice, Notice::Connection(ConnectionChange::Disconnected { .. }))));
    append(&log, run_initialized);
    collect_until(&tracker, |seen| kinds(seen).contains(&"RunStarted"));
    tracker.stop();
    assert_eq!(*saved_events.lock().unwrap(), ["RunStarted", "RunStarted"]);
}

#[test]
fn a_rebinding_is_applied_to_the_history() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Player.log");
    append(&log, "old\n");
    let history = Arc::new(History::open(&dir.path().join("history.db")).unwrap());
    let memory = FakeMemory::new();
    memory.set_board(board());
    let tracker = Tracker::start(config(&log), Box::new(memory.clone()), Box::new(history.clone()), Arc::new(SystemClock), None);
    std::thread::sleep(Duration::from_millis(100));

    // No run id in memory yet: the run starts under a local id.
    append(&log, "[00:00:00.000] [HttpGameClient] Command completed: type=RerollCommand, requestId=1, status=200 OK, durationMs=5, result=success, endpoint=/commands\n");
    let notices = collect_until(&tracker, |seen| kinds(seen).len() == 2);
    let local = saved(&notices)[0].run_id.clone();
    assert!(local.starts_with("local-"));

    // Memory learns the real id; the next line notices.
    memory.set_run_init(RunInit { run_id: Some("run-real-2".into()), ..RunInit::default() });
    append(&log, "[00:00:00.000] [AppState] State changed from [StartRunAppState] to [ChoiceState]\n");
    let notices = collect_until(&tracker, |seen| seen.iter().any(|notice| matches!(notice, Notice::RunRebound { .. })));
    tracker.stop();

    assert!(notices.contains(&Notice::RunRebound { old_run_id: local.clone(), new_run_id: "run-real-2".into() }));
    assert!(history.get_run(&local).unwrap().is_none());
    assert_eq!(history.list_events("run-real-2").unwrap().len(), 2);
}

fn live_board(gold: i32) -> BoardState {
    let mut board = board();
    board.player.stats.gold = Some(gold);
    board
}

fn live_golds(notices: &[Notice]) -> Vec<Option<i32>> {
    notices
        .iter()
        .filter_map(|notice| match notice {
            Notice::LiveSnapshot(board) => Some(board.as_ref().and_then(|board| board.player.stats.gold)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_live_board_is_polled_while_attached_and_sent_only_when_it_changes() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Player.log");
    append(&log, "old\n");
    let memory = FakeMemory::new();
    memory.set_board(live_board(10));
    let history = Arc::new(History::open(&dir.path().join("history.db")).unwrap());
    let (changes, connection) = mpsc::channel();
    let clock = ManualClock::new(0);
    let mut config = config(&log);
    config.live_interval = Some(Duration::from_millis(1000));
    let tracker = Tracker::start(config, Box::new(memory.clone()), Box::new(history.clone()), Arc::new(clock.clone()), Some(connection));

    // Detached: nothing is read, however much time passes.
    clock.advance(5000);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(memory.board_calls(), 0);

    changes.send(ConnectionChange::Connected { pid: 1, unity_version: "test".into() }).unwrap();
    let seen = collect_until(&tracker, |seen| !live_golds(seen).is_empty());
    assert_eq!(live_golds(&seen), [Some(10)]);

    // An unchanged board is not sent again, however often it is read.
    let reads = memory.board_calls();
    clock.advance(1000);
    std::thread::sleep(Duration::from_millis(150));
    assert!(memory.board_calls() > reads, "it was read again");
    assert_eq!(tracker.notices().try_iter().filter(|notice| matches!(notice, Notice::LiveSnapshot(_))).count(), 0);

    // A changed one is.
    memory.set_board(live_board(11));
    clock.advance(1000);
    let seen = collect_until(&tracker, |seen| !live_golds(seen).is_empty());
    assert_eq!(live_golds(&seen), [Some(11)]);

    // No run (nothing to read) is reported once, as `None`.
    memory.set_board(None);
    clock.advance(1000);
    let seen = collect_until(&tracker, |seen| !live_golds(seen).is_empty());
    assert_eq!(live_golds(&seen), [None]);

    // After the game is lost nothing more is read, and nothing of it is saved.
    changes.send(ConnectionChange::Disconnected { reason: "gone".into() }).unwrap();
    collect_until(&tracker, |seen| seen.iter().any(|notice| matches!(notice, Notice::Connection(ConnectionChange::Disconnected { .. }))));
    let reads = memory.board_calls();
    clock.advance(10_000);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(memory.board_calls(), reads);
    tracker.stop();
    assert!(history.list_runs().unwrap().is_empty(), "live reads are never saved");
}

#[test]
fn with_the_live_interval_off_the_board_is_not_read_by_the_service() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("Player.log");
    append(&log, "old\n");
    let memory = FakeMemory::new();
    memory.set_board(live_board(10));
    let (changes, connection) = mpsc::channel();
    let store = Flaky { failures: 0, calls: 0, saved: Arc::new(Mutex::new(Vec::new())) };
    let tracker = Tracker::start(config(&log), Box::new(memory.clone()), Box::new(store), Arc::new(ManualClock::new(0)), Some(connection));
    changes.send(ConnectionChange::Connected { pid: 1, unity_version: "test".into() }).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(memory.board_calls(), 0);
    tracker.stop();
}
