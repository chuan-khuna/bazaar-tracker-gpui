//! Follow the live game headlessly and print each Event as it is saved.
//!
//! `just track [-- --log <Player.log>]`. It saves to `history.db` in the app's data folder, found as
//! the app does: `BAZAAR_TRACKER_DATA_DIR`, else `.bazaar-tracker/` in the repo for a debug build,
//! else `%APPDATA%\BazaarTracker\`. Stop it with Ctrl+C.

use std::path::PathBuf;
use std::sync::Arc;

use bazaar_tracker::history::{self, History};
use bazaar_tracker::log::path as log_path;
use bazaar_tracker::tracker::{Notice, Tracker, TrackerConfig};

/// The data folder, resolved as `src/shell/data_dir.rs` of the app does it.
fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BAZAAR_TRACKER_DATA_DIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(".bazaar-tracker");
    }
    std::env::var_os("APPDATA").map_or_else(|| PathBuf::from("BazaarTracker"), |dir| PathBuf::from(dir).join("BazaarTracker"))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut log_setting = String::new();
    while let Some(arg) = args.next() {
        if arg == "--log" {
            log_setting = args.next().unwrap_or_default();
        }
    }

    let dir = data_dir();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("track: cannot create {}: {error}", dir.display());
        std::process::exit(1);
    }
    let history = match History::open(&dir.join(history::FILE_NAME)) {
        Ok(history) => Arc::new(history),
        Err(error) => {
            eprintln!("track: {error}");
            std::process::exit(1);
        }
    };
    let log = log_path::resolve(&log_setting, log_path::user_profile().as_deref());
    println!("following {}", log.display());
    println!("saving to {}", dir.join(history::FILE_NAME).display());

    let tracker = Tracker::start_live(TrackerConfig::new(log), history);
    for notice in tracker.notices() {
        match notice {
            Notice::Connection(change) => println!("[connection] {change:?}"),
            Notice::EventSaved(event) => println!("[saved] {}", serde_json::to_string(&event).unwrap_or_else(|_| format!("{event:?}"))),
            Notice::RunRebound { old_run_id, new_run_id } => println!("[rebound] {old_run_id} -> {new_run_id}"),
            Notice::SaveFailed { what, error, retry_in } => println!("[save failed] {what}: {error}; trying again in {retry_in:?}"),
            Notice::LogError(error) => println!("[log error] {error}"),
            Notice::LiveSnapshot(_) => {}
        }
    }
}
