//! The tracker service, started on a background thread, and the bridge that hands its notices to
//! the window.
//!
//! `bazaar-tracker`'s `Tracker` reports over a blocking `std::sync::mpsc` channel; the window waits
//! on an `async_channel`. One thread owns the `Tracker` and passes each notice from the one to the
//! other. Dropping the [`Service`] stops that thread, which drops the `Tracker`, which stops and
//! joins its own threads.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use bazaar_tracker::tracker::Notice;

/// How often the bridge looks at its stop flag while nothing is being reported.
const STOP_CHECK: Duration = Duration::from_millis(50);

/// The running tracker, or the reason there is none.
pub struct Service {
    notices: async_channel::Receiver<Notice>,
    /// Why the service could not start, if it could not.
    pub error: Option<String>,
    running: bool,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Service {
    /// Open `history.db` in `data_dir`, and follow the game, saving to it. `log_setting` is the
    /// `log_path` setting. Never fails: a service that could not start has `error` set, reports
    /// nothing, and is not [`running`](Self::running).
    pub fn start(data_dir: &Path, log_setting: &str) -> Self {
        match Self::try_start(data_dir, log_setting) {
            Ok(service) => service,
            Err(error) => {
                tracing::error!("the tracker did not start: {error}");
                let (_, notices) = async_channel::unbounded();
                Self { notices, error: Some(error), running: false, stop: Arc::new(AtomicBool::new(false)), thread: None }
            }
        }
    }

    #[cfg(windows)]
    fn try_start(data_dir: &Path, log_setting: &str) -> Result<Self, String> {
        use bazaar_tracker::history::{self, History};
        use bazaar_tracker::log::path as log_path;
        use bazaar_tracker::tracker::{Tracker, TrackerConfig};

        let history = History::open(&data_dir.join(history::FILE_NAME)).map_err(|error| format!("Could not open {}: {error}", history::FILE_NAME))?;
        let log = log_path::resolve(log_setting, log_path::user_profile().as_deref());
        tracing::info!("following {}", log.display());
        let tracker = Tracker::start_live(TrackerConfig::new(log), Arc::new(history));
        Ok(Self::bridge(tracker))
    }

    #[cfg(not(windows))]
    fn try_start(_data_dir: &Path, _log_setting: &str) -> Result<Self, String> {
        Err("Tracking is only available on Windows.".into())
    }

    #[cfg(windows)]
    fn bridge(tracker: bazaar_tracker::tracker::Tracker) -> Self {
        let (out, notices) = async_channel::unbounded();
        let stop = Arc::new(AtomicBool::new(false));
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
        Self { notices, error: None, running: true, stop, thread: Some(thread) }
    }

    /// Whether the service started.
    pub fn running(&self) -> bool {
        self.running
    }

    /// What the service reports, in the order it happened. Awaiting never blocks the window.
    pub fn notices(&self) -> async_channel::Receiver<Notice> {
        self.notices.clone()
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
