//! The schema, and the migrations that bring a file up to date.

use std::path::{Path, PathBuf};

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
    // 5: the items the player tracks in a Run, to see their odds beside each merchant.
    "
    CREATE TABLE tracked_items (
        run_id      TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE ON UPDATE CASCADE,
        template_id TEXT NOT NULL,
        added_ms    INTEGER NOT NULL,
        PRIMARY KEY (run_id, template_id)
    );
    ",
    // 6: the items the player hid from the odds beside each merchant in a Run.
    "
    CREATE TABLE hidden_items (
        run_id      TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE ON UPDATE CASCADE,
        template_id TEXT NOT NULL,
        hidden_ms   INTEGER NOT NULL,
        PRIMARY KEY (run_id, template_id)
    );
    ",
    // 7: the items the player pins for a hero, tracked in each of that hero's Runs.
    "
    CREATE TABLE pinned_items (
        hero        TEXT NOT NULL,
        template_id TEXT NOT NULL,
        pinned_ms   INTEGER NOT NULL,
        PRIMARY KEY (hero, template_id)
    );
    ",
    // 8: prefix every table by where its rows come from (docs/records/2026-11-08.002): run_tracker_
    // for what the tracker records, imported_ for what comes from outside, setting_ for the player's
    // choices. A rename rewrites the child tables' REFERENCES; indexes keep their old names, so they
    // are made again. The odds say where each row came from, and when it was imported.
    "
    ALTER TABLE runs          RENAME TO run_tracker_runs;
    ALTER TABLE events        RENAME TO run_tracker_events;
    ALTER TABLE tracked_items RENAME TO run_tracker_tracked_items;
    ALTER TABLE hidden_items  RENAME TO run_tracker_hidden_items;
    ALTER TABLE shop_odds     RENAME TO imported_shop_odds;
    ALTER TABLE item_odds     RENAME TO imported_item_odds;
    ALTER TABLE pinned_items  RENAME TO setting_pinned_items;

    DROP INDEX runs_by_start;
    DROP INDEX events_by_run;
    DROP INDEX events_waiting_for_upload;
    CREATE INDEX run_tracker_runs_by_start ON run_tracker_runs (started_ms);
    CREATE INDEX run_tracker_events_by_run ON run_tracker_events (run_id, timestamp_ms, seq);
    CREATE INDEX run_tracker_events_waiting_for_upload ON run_tracker_events (seq) WHERE upload_status = 0;

    ALTER TABLE imported_shop_odds RENAME COLUMN saved_ms TO imported_ms;
    ALTER TABLE imported_shop_odds ADD COLUMN source TEXT NOT NULL DEFAULT 'bazaardb';
    ALTER TABLE imported_item_odds RENAME COLUMN saved_ms TO imported_ms;
    ALTER TABLE imported_item_odds ADD COLUMN source TEXT NOT NULL DEFAULT 'bazaardb';
    ",
];

/// The version a file has after all the migrations.
pub const CURRENT_VERSION: i64 = MIGRATIONS.len() as i64;

/// The schema version of `connection`'s file; 0 for a new one.
pub fn version_of(connection: &Connection) -> Result<i64> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Copy the file `connection` has open, at schema `version`, to a new file beside `path` named
/// `history.v<version>.bak.db` (or `.bak.2.db`, `.bak.3.db`, … when that is taken), and return
/// where. The copy is made with `VACUUM INTO`, so it is whole and consistent even in WAL mode.
pub fn back_up(connection: &Connection, path: &Path, version: i64) -> Result<PathBuf> {
    let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("history");
    let name = |n: u32| if n == 1 { format!("{stem}.v{version}.bak.db") } else { format!("{stem}.v{version}.bak.{n}.db") };
    let backup = (1..).map(|n| path.with_file_name(name(n))).find(|candidate| !candidate.exists()).expect("some name is free");
    connection.execute("VACUUM INTO ?1", [backup.to_string_lossy()])?;
    Ok(backup)
}

/// Bring `connection`'s file up to date with `migrations`: each one not yet applied is run, in a
/// transaction with the version bump, and never again. A file ahead of `migrations` is refused.
pub fn migrate_with(connection: &mut Connection, migrations: &[&str]) -> Result<()> {
    let version = version_of(connection)?;
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
