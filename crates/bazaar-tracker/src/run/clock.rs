//! Time, as the run state machine sees it. The machine never reads the system clock itself, so
//! tests drive its timers by moving a [`ManualClock`].

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A source of "now", in milliseconds since the Unix epoch.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

/// The real time.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64)
    }
}

/// A clock that only moves when told to. Clones share the same time.
#[derive(Clone, Debug)]
pub struct ManualClock {
    now: Arc<AtomicI64>,
}

impl ManualClock {
    pub fn new(start_ms: i64) -> Self {
        Self { now: Arc::new(AtomicI64::new(start_ms)) }
    }

    pub fn advance(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn set(&self, ms: i64) {
        self.now.store(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }
}
