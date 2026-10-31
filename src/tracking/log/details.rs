//! The fixed block of facts at the top of the log panel: the game and how it is attached, where
//! the tracker reads and writes, and when it last saw something.

use std::path::PathBuf;

use bazaar_tracker::memory::{AttachDetails, ConnectionChange};
use bazaar_tracker::tracker::Notice;

use crate::tracking::when;

/// The process the tracker looks for, shown while it is not attached.
const PROCESS_NAME: &str = "TheBazaar";
const UNKNOWN: &str = "—";

#[derive(Clone, Debug)]
pub struct Details {
    player_log: PathBuf,
    history_db: PathBuf,
    /// Whether the tracker service started.
    running: bool,
    attached: Option<(u32, AttachDetails)>,
    /// When the last Event was saved, in milliseconds since the Unix epoch.
    last_activity_ms: Option<i64>,
}

impl Details {
    pub fn new(player_log: PathBuf, history_db: PathBuf, running: bool) -> Self {
        Self { player_log, history_db, running, attached: None, last_activity_ms: None }
    }

    /// Take a notice in: attaching and losing the game, and Events saved.
    pub fn apply(&mut self, notice: &Notice) {
        match notice {
            Notice::Connection(ConnectionChange::Connected { pid, details, .. }) => self.attached = Some((*pid, details.clone())),
            Notice::Connection(ConnectionChange::Disconnected { .. }) => self.attached = None,
            Notice::EventSaved(event) => self.last_activity_ms = Some(event.timestamp_ms),
            _ => {}
        }
    }

    /// The label and value of each line, in order.
    pub fn lines(&self) -> Vec<(&'static str, String)> {
        let state = match (&self.attached, self.running) {
            (Some(_), _) => "Attached",
            (None, true) => "Waiting for the game",
            (None, false) => "The tracker is not running",
        };
        let (process, unity, offsets, mono, domain) = match &self.attached {
            Some((pid, details)) => (
                format!("{} (pid {pid})", name_of(details)),
                or_unknown(&details.unity_version),
                format!("{} · {}", or_unknown(&details.offsets), if details.offsets_verified { "verified" } else { "not verified" }),
                format!("{} at {}", or_unknown(&details.mono_module), hex(details.mono_module_base)),
                hex(details.root_domain),
            ),
            None => (PROCESS_NAME.to_owned(), UNKNOWN.to_owned(), UNKNOWN.to_owned(), UNKNOWN.to_owned(), UNKNOWN.to_owned()),
        };
        vec![
            ("State", state.to_owned()),
            ("Process", process),
            ("Unity", unity),
            ("Offsets", offsets),
            ("Mono module", mono),
            ("Root domain", domain),
            ("Player.log", self.player_log.display().to_string()),
            ("history.db", self.history_db.display().to_string()),
            ("Last event", self.last_activity_ms.map_or_else(|| "none yet".to_owned(), when::hms)),
        ]
    }
}

fn name_of(details: &AttachDetails) -> &str {
    if details.process_name.is_empty() { PROCESS_NAME } else { &details.process_name }
}

fn or_unknown(text: &str) -> String {
    if text.is_empty() { UNKNOWN.to_owned() } else { text.to_owned() }
}

fn hex(address: u64) -> String {
    if address == 0 { UNKNOWN.to_owned() } else { format!("0x{address:x}") }
}

#[cfg(test)]
#[path = "details_tests.rs"]
mod tests;
