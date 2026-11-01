//! The schema, and the migrations that bring a file up to date.

use rusqlite::Connection;

use super::{Error, Result};

/// The migrations, in order. Migration number N takes the file from `user_version` N-1 to N. Add new
/// ones at the end; never change one that has shipped.
pub const MIGRATIONS: &[&str] = &[
    // 1: the first schema.
    "
    CREATE TABLE runs (
        id          TEXT PRIMARY KEY,
        started_ms  INTEGER NOT NULL,
        ended_ms    INTEGER,
        ended       INTEGER NOT NULL DEFAULT 0,
        hero        TEXT,
        day         INTEGER,
        victories   INTEGER,
        losses      INTEGER,
        outcome     TEXT
    );
    CREATE TABLE events (
        seq          INTEGER PRIMARY KEY AUTOINCREMENT,
        id           TEXT NOT NULL UNIQUE,
        run_id       TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE ON UPDATE CASCADE,
        kind         TEXT NOT NULL,
        timestamp_ms INTEGER NOT NULL,
        day          INTEGER,
        hour         INTEGER,
        details      TEXT NOT NULL
    );
    CREATE INDEX events_by_run ON events (run_id, timestamp_ms, seq);
    CREATE INDEX runs_by_start ON runs (started_ms);
    ",
    // 2: which Events have been uploaded, and which the server refused for good.
    "
    ALTER TABLE events ADD COLUMN upload_status INTEGER NOT NULL DEFAULT 0;
    CREATE INDEX events_waiting_for_upload ON events (seq) WHERE upload_status = 0;
    ",
    // 3: Bazaar DB's shop odds, one row for each day, and when they were saved.
    "
    CREATE TABLE shop_odds (
        day        INTEGER PRIMARY KEY,
        samples    INTEGER NOT NULL,
        bronze     REAL NOT NULL,
        silver     REAL NOT NULL,
        gold       REAL NOT NULL,
        diamond    REAL NOT NULL,
        saved_ms   INTEGER NOT NULL
    );
    ",
    // 4: Bazaar DB's odds for one item, for a hero and the tier it is owned at, kept as answered.
    "
    CREATE TABLE item_odds (
        template_id TEXT NOT NULL,
        hero        TEXT NOT NULL,
        tier        TEXT NOT NULL,
        url_id      TEXT NOT NULL,
        item_name   TEXT NOT NULL,
        body        TEXT NOT NULL,
        saved_ms    INTEGER NOT NULL,
        PRIMARY KEY (template_id, hero, tier)
    );
    ",
];

/// The version a file has after all the migrations.
pub const CURRENT_VERSION: i64 = MIGRATIONS.len() as i64;

/// Bring `connection`'s file up to date with `migrations`: each one not yet applied is run, in a
/// transaction with the version bump, and never again. A file ahead of `migrations` is refused.
pub fn migrate_with(connection: &mut Connection, migrations: &[&str]) -> Result<()> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let supported = migrations.len() as i64;
    if version > supported {
        return Err(Error::NewerSchema { found: version, supported });
    }
    for (index, sql) in migrations.iter().enumerate().skip(version as usize) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        // `user_version` cannot take a bound parameter.
        transaction.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
        transaction.commit()?;
    }
    Ok(())
}
