//! `history.db`: Runs and Events, in our own schema (not the original tracker's).
//!
//! - Tables `runs` and `events`; each Event's details are JSON, its times integer milliseconds
//!   since the Unix epoch, and foreign keys are on.
//! - The schema version is SQLite's `user_version`. A new file gets the current schema; an older one
//!   is migrated forward, once; a newer one is refused.
//! - Inserting an Event updates its Run's summary in the same transaction.
//! - Writes go through one connection behind a lock; reads use another, and the database is in WAL
//!   mode, so listing runs does not wait for a write.

mod schema;
mod shop_odds;
mod uploads;

/// The name of the file in the data folder.
pub const FILE_NAME: &str = "history.db";

use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::run::{Event, EventKind, Outcome, Run};

pub use schema::{CURRENT_VERSION, MIGRATIONS, migrate_with};
pub use shop_odds::SavedShopOdds;
pub use uploads::{UploadStatus, WaitingEvent};

/// What went wrong with the history.
#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    /// The file was written by a newer version of the app.
    NewerSchema { found: i64, supported: i64 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "history database error: {error}"),
            Self::Json(error) => write!(f, "history event details are not valid JSON: {error}"),
            Self::NewerSchema { found, supported } => write!(f, "history.db has schema version {found}, but this app only understands up to {supported}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::NewerSchema { .. } => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The history store. Share it between threads by reference or `Arc`.
pub struct History {
    writer: Mutex<Connection>,
    reader: Mutex<Connection>,
}

fn open_connection(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    Ok(connection)
}

fn lock(connection: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    connection.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl History {
    /// Open (creating if need be) the history at `path`, bringing its schema up to date.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_migrations(path, MIGRATIONS)
    }

    /// As [`Self::open`] with a chosen list of migrations; each is run once, in order, on a file
    /// that has not had it yet.
    pub fn open_with_migrations(path: &Path, migrations: &[&str]) -> Result<Self> {
        let mut writer = open_connection(path)?;
        writer.pragma_update(None, "journal_mode", "WAL")?;
        migrate_with(&mut writer, migrations)?;
        let reader = open_connection(path)?;
        reader.pragma_update(None, "query_only", true)?;
        Ok(Self { writer: Mutex::new(writer), reader: Mutex::new(reader) })
    }

    /// Run `f` on the write connection, for the other parts of the crate that extend the schema.
    pub(crate) fn with_writer<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        f(&mut lock(&self.writer))
    }

    pub(crate) fn with_reader<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        f(&lock(&self.reader))
    }

    /// The schema version of the open file.
    pub fn schema_version(&self) -> Result<i64> {
        self.with_reader(|connection| Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?))
    }

    /// Save an Event, creating its Run if this is the first Event of it, and update the Run's
    /// summary, in one transaction. Saving the same Event again changes nothing and returns `false`.
    pub fn insert_event(&self, event: &Event) -> Result<bool> {
        let details = serde_json::to_string(&event.kind)?;
        self.with_writer(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute("INSERT OR IGNORE INTO runs (id, started_ms) VALUES (?1, ?2)", params![event.run_id, event.timestamp_ms])?;
            let added = transaction.execute(
                "INSERT OR IGNORE INTO events (id, run_id, kind, timestamp_ms, day, hour, details) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![event.id, event.run_id, event.kind.name(), event.timestamp_ms, event.day, event.hour, details],
            )?;
            if added > 0 {
                update_summary(&transaction, event)?;
            }
            transaction.commit()?;
            Ok(added > 0)
        })
    }

    /// Every Run, newest first.
    pub fn list_runs(&self) -> Result<Vec<Run>> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare(&format!("SELECT {RUN_COLUMNS} FROM runs ORDER BY started_ms DESC, rowid DESC"))?;
            let runs = statement.query_map([], run_from_row)?.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(runs)
        })
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<Run>> {
        self.with_reader(|connection| Ok(connection.query_row(&format!("SELECT {RUN_COLUMNS} FROM runs WHERE id = ?1"), [run_id], run_from_row).optional()?))
    }

    /// The Events of a Run, in the order they happened.
    pub fn list_events(&self, run_id: &str) -> Result<Vec<Event>> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare(&format!("SELECT {EVENT_COLUMNS} FROM events WHERE run_id = ?1 ORDER BY timestamp_ms, seq"))?;
            let rows = statement.query_map([run_id], event_row)?.collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().map(event_from_row).collect()
        })
    }

    pub fn load_event(&self, event_id: &str) -> Result<Option<Event>> {
        self.with_reader(|connection| {
            let row = connection.query_row(&format!("SELECT {EVENT_COLUMNS} FROM events WHERE id = ?1"), [event_id], event_row).optional()?;
            row.map(event_from_row).transpose()
        })
    }

    /// Delete a Run and its Events. Returns whether there was such a Run.
    pub fn delete_run(&self, run_id: &str) -> Result<bool> {
        self.with_writer(|connection| Ok(connection.execute("DELETE FROM runs WHERE id = ?1", [run_id])? > 0))
    }

    /// Mark a Run ended or not ended (a Run that is started again after it ended is not ended).
    pub fn set_run_ended(&self, run_id: &str, ended: bool) -> Result<bool> {
        self.with_writer(|connection| {
            let sql = if ended { "UPDATE runs SET ended = 1 WHERE id = ?1" } else { "UPDATE runs SET ended = 0, ended_ms = NULL, outcome = NULL WHERE id = ?1" };
            Ok(connection.execute(sql, [run_id])? > 0)
        })
    }

    /// Give the Run `old_id` the id `new_id`, and move its Events with it, in one transaction.
    /// If a Run with `new_id` exists already, the two are merged into it. Returns whether `old_id`
    /// existed.
    pub fn rebind_run(&self, old_id: &str, new_id: &str) -> Result<bool> {
        if old_id == new_id {
            return Ok(false);
        }
        self.with_writer(|connection| {
            let transaction = connection.transaction()?;
            let exists = |id: &str| -> rusqlite::Result<bool> { transaction.query_row("SELECT EXISTS (SELECT 1 FROM runs WHERE id = ?1)", [id], |row| row.get(0)) };
            if !exists(old_id)? {
                return Ok(false);
            }
            if exists(new_id)? {
                // Merge: the events join the existing run, which keeps the earlier start.
                transaction.execute("UPDATE events SET run_id = ?2 WHERE run_id = ?1", params![old_id, new_id])?;
                transaction.execute(
                    "UPDATE runs SET started_ms = MIN(started_ms, (SELECT started_ms FROM runs WHERE id = ?1)) WHERE id = ?2",
                    params![old_id, new_id],
                )?;
                transaction.execute("DELETE FROM runs WHERE id = ?1", [old_id])?;
            } else {
                // The events follow by the foreign key's ON UPDATE CASCADE.
                transaction.execute("UPDATE runs SET id = ?2 WHERE id = ?1", params![old_id, new_id])?;
            }
            transaction.commit()?;
            Ok(true)
        })
    }
}

const RUN_COLUMNS: &str = "id, started_ms, ended_ms, ended, hero, day, victories, losses, outcome";
const EVENT_COLUMNS: &str = "id, run_id, timestamp_ms, day, hour, details";

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
    let outcome: Option<String> = row.get(8)?;
    Ok(Run {
        id: row.get(0)?,
        started_ms: row.get(1)?,
        ended_ms: row.get(2)?,
        ended: row.get(3)?,
        hero: row.get(4)?,
        day: row.get(5)?,
        victories: row.get(6)?,
        losses: row.get(7)?,
        outcome: outcome.as_deref().map(outcome_from_text),
    })
}

fn outcome_from_text(text: &str) -> Outcome {
    match text {
        "Win" => Outcome::Win,
        "Loss" => Outcome::Loss,
        _ => Outcome::Unknown,
    }
}

type EventRow = (String, String, i64, Option<i32>, Option<i32>, String);

fn event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
}

fn event_from_row((id, run_id, timestamp_ms, day, hour, details): EventRow) -> Result<Event> {
    Ok(Event { id, run_id, timestamp_ms, day, hour, kind: serde_json::from_str(&details)? })
}

/// Fold an Event into its Run's summary.
fn update_summary(transaction: &Transaction<'_>, event: &Event) -> Result<()> {
    let run_id = event.run_id.as_str();
    if let Some(day) = event.day {
        transaction.execute("UPDATE runs SET day = MAX(COALESCE(day, 0), ?2) WHERE id = ?1", params![run_id, day])?;
    }
    match &event.kind {
        EventKind::RunStarted { hero, .. } => {
            // Starting a run again (as when the tracker restarts mid-run) clears its end.
            transaction.execute(
                "UPDATE runs SET started_ms = ?2, ended = 0, ended_ms = NULL, outcome = NULL, hero = COALESCE(?3, hero) WHERE id = ?1",
                params![run_id, event.timestamp_ms, hero],
            )?;
        }
        EventKind::CombatEnd { victories_after, losses_after, .. } => {
            transaction.execute("UPDATE runs SET victories = COALESCE(?2, victories), losses = COALESCE(?3, losses) WHERE id = ?1", params![run_id, victories_after, losses_after])?;
        }
        EventKind::RunEnded { outcome, victories, losses, .. } => {
            transaction.execute(
                "UPDATE runs SET ended = 1, ended_ms = ?2, outcome = ?3, victories = COALESCE(?4, victories), losses = COALESCE(?5, losses) WHERE id = ?1",
                params![run_id, event.timestamp_ms, outcome.as_str(), victories, losses],
            )?;
        }
        _ => {}
    }
    // Any Event whose Snapshot names the hero fills it in if the run has none yet.
    let snapshot_hero = match &event.kind {
        EventKind::OptionsPresented { snapshot, .. } | EventKind::Selected { snapshot, .. } | EventKind::Sold { snapshot, .. } => snapshot.stats.hero.as_deref(),
        EventKind::CombatStart { player, .. } => player.stats.hero.as_deref(),
        _ => None,
    };
    if let Some(hero) = snapshot_hero {
        transaction.execute("UPDATE runs SET hero = ?2 WHERE id = ?1 AND hero IS NULL", params![run_id, hero])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
