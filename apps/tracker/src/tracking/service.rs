//! The tracker service, started on a background thread, and the bridge that hands its notices to
//! the window.
//!
//! `bazaar-tracker`'s `Tracker` reports over a blocking `std::sync::mpsc` channel; the window waits
//! on an `async_channel`. One thread owns the `Tracker` and passes each notice from the one to the
//! other. Dropping the [`Service`] stops that thread, which drops the `Tracker`, which stops and
//! joins its own threads.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use bazaar_tracker::history::{self, History};
use bazaar_tracker::log::path as log_path;
use bazaar_tracker::tracker::{Control, Notice};

/// How often the bridge looks at its stop flag while nothing is being reported.
const STOP_CHECK: Duration = Duration::from_millis(50);

/// The running tracker, or the reason there is none.
pub struct Service {
    notices: async_channel::Receiver<Notice>,
    history: Option<Arc<History>>,
    control: Option<Control>,
    /// Why the service could not start, if it could not.
    pub error: Option<String>,
    /// The `Player.log` it follows (or would), and the `history.db` it saves to (or would).
    player_log: PathBuf,
    history_db: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Service {
    /// Open `history.db` in `data_dir`, and follow the game, saving to it. `log_setting` is the
    /// `log_path` setting. Never fails: a service that could not start has `error` set, reports
    /// nothing, and has no history in its [`link`](Self::link).
    pub fn start(data_dir: &Path, log_setting: &str) -> Self {
        let player_log = log_path::resolve(log_setting, log_path::user_profile().as_deref());
        let history_db = data_dir.join(history::FILE_NAME);
        match Self::try_start(&player_log, &history_db) {
            Ok(mut service) => {
                service.player_log = player_log;
                service.history_db = history_db;
                service
            }
            Err(error) => {
                tracing::error!("the tracker did not start: {error}");
                let (_, notices) = async_channel::unbounded();
                Self { notices, history: None, control: None, error: Some(error), player_log, history_db, stop: Arc::new(AtomicBool::new(false)), thread: None }
            }
        }
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn try_start(player_log: &Path, history_db: &Path) -> Result<Self, String> {
        use bazaar_tracker::tracker::{Tracker, TrackerConfig};

        let history = History::open(history_db).map_err(|error| format!("Could not open {}: {error}", history::FILE_NAME))?;
        let history = Arc::new(history);
        tracing::info!("following {}", player_log.display());
        let tracker = Tracker::start_live(TrackerConfig::new(player_log), history.clone());
        Ok(Self::bridge(tracker, history))
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    fn try_start(_player_log: &Path, _history_db: &Path) -> Result<Self, String> {
        Err("Tracking is only available on Windows and macOS.".into())
    }

    #[cfg(any(windows, target_os = "macos"))]
    fn bridge(tracker: bazaar_tracker::tracker::Tracker, history: Arc<History>) -> Self {
        let (out, notices) = async_channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let control = tracker.control();
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("tracker-bridge".into())
                .spawn(move || {
                    forward(tracker.notices(), &out, &stop);
                    drop(tracker);
                })
                .expect("the operating system can start a thread")
        };
        Self { notices, history: Some(history), control: Some(control), error: None, player_log: PathBuf::new(), history_db: PathBuf::new(), stop, thread: Some(thread) }
    }

    /// What the window needs of the service.
    pub fn link(&self) -> Link {
        Link { notices: self.notices.clone(), history: self.history.clone(), control: self.control.clone(), player_log: self.player_log.clone(), history_db: self.history_db.clone() }
    }
}

/// What the window holds of the service: what it reports, and the history it saves to.
pub struct Link {
    /// What the service reports, in the order it happened. Awaiting never blocks the window.
    pub notices: async_channel::Receiver<Notice>,
    /// The history the service saves to, which the window reads. `None` if the service did not start.
    pub history: Option<Arc<History>>,
    /// Asks the service for things (picking up the game's run). `None` if the service did not start.
    pub control: Option<Control>,
    /// Where `Player.log` is followed, and where `history.db` is.
    pub player_log: PathBuf,
    pub history_db: PathBuf,
}

impl Link {
    /// Whether the service started.
    pub fn running(&self) -> bool {
        self.history.is_some()
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Pass notices from `source` to `out`, in order, until `stop` is set, `source` is closed or
/// nothing is listening to `out` any more.
pub fn forward(source: &Receiver<Notice>, out: &async_channel::Sender<Notice>, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match source.recv_timeout(STOP_CHECK) {
            Ok(notice) => {
                if out.send_blocking(notice).is_err() {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
