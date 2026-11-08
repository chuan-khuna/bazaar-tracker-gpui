# History database

How `crates/bazaar-tracker/src/history/` keeps Runs and Events, and the Shop odds and item odds
saved from Bazaar DB, in `history.db`. The terms (Run, Live run, Event, Snapshot, Shop odds) are
defined in [GLOSSARY.md](../../GLOSSARY.md). The Events themselves come from the
[run state machine](run-state-machine.md).

## Files

| File | What it does |
|---|---|
| `history/mod.rs` | `History`: opening, the two connections, saving Events, the Run summary, listing, deleting, rebinding; `Error` |
| `history/schema.rs` | `MIGRATIONS`, `CURRENT_VERSION`, `migrate_with`, `back_up` |
| `history/uploads.rs` | `upload_status` on `run_tracker_events`: `UploadStatus`, `waiting_events`, `set_upload_status` |
| `history/shop_odds.rs` | The `imported_shop_odds` table: `SavedShopOdds`, `shop_odds`, `save_shop_odds`, `seed_shop_odds` |
| `history/item_odds.rs` | The `imported_item_odds` table: `SavedItemOdds`, `item_odds`, `save_item_odds`, `import_item_odds` |
| `history/tracked_items.rs` | The `run_tracker_tracked_items` table: `tracked_items`, `track_item`, `untrack_item` |
| `history/hidden_items.rs` | The `run_tracker_hidden_items` table: `hidden_items`, `hide_item`, `unhide_item` |
| `history/pinned_items.rs` | The `setting_pinned_items` table: `pinned_items`, `pin_item`, `unpin_item` |

The schema is our own, not the original tracker's.

## The file and its connections

The file is `history.db` (`history::FILE_NAME`) in the data folder (see
[data-folder.md](data-folder.md)): `.bazaar-tracker/history.db` for a debug build,
`%APPDATA%\BazaarTracker\history.db` for a release one.

`History::open(path)` creates the file if it is missing and brings its schema up to date. It holds
two connections, each behind a `Mutex`:

| Connection | Set up with | Used for |
|---|---|---|
| `writer` | `journal_mode = WAL`, then the migrations | Every write (`with_writer`) |
| `reader` | `query_only = true` | Every read (`with_reader`) |

Both have `foreign_keys` on and a 5 s busy timeout. Because the file is in WAL mode, a read (such
as listing Runs for the window) does not wait for a write (such as the tracker saving an Event).
Writes from several threads queue on the writer lock. A lock poisoned by a panic is taken anyway.
Share a `History` by reference or `Arc`; the tracker's `Store` is `Arc<History>`.

All times are integer milliseconds since the Unix epoch: `started_ms`, `ended_ms`, `timestamp_ms`,
`imported_ms`, `added_ms`, `hidden_ms`.

## Tables

### `run_tracker_runs`

One row for each Run: the summary the sidebar lists.

| Column | Type | What it holds |
|---|---|---|
| `id` | `TEXT PRIMARY KEY` | The game's run id, or a `local-` id until it is known |
| `started_ms` | `INTEGER NOT NULL` | When the Run started |
| `ended_ms` | `INTEGER` | When it ended; `NULL` while it has not |
| `ended` | `INTEGER NOT NULL DEFAULT 0` | 1 once ended |
| `hero` | `TEXT` | The hero |
| `day` | `INTEGER` | The furthest day reached |
| `victories` | `INTEGER` | Wins so far, or at the end |
| `losses` | `INTEGER` | Losses so far, or at the end |
| `outcome` | `TEXT` | `Win`, `Loss` or `Unknown`; `NULL` while not ended |

Index `run_tracker_runs_by_start` on `(started_ms)`.

### `run_tracker_events`

One row for each Event.

| Column | Type | What it holds |
|---|---|---|
| `seq` | `INTEGER PRIMARY KEY AUTOINCREMENT` | Insertion order; breaks ties between equal times |
| `id` | `TEXT NOT NULL UNIQUE` | The Event's id |
| `run_id` | `TEXT NOT NULL` | `REFERENCES run_tracker_runs(id) ON DELETE CASCADE ON UPDATE CASCADE` |
| `kind` | `TEXT NOT NULL` | The `type` tag: `RunStarted`, `OptionsPresented`, `Selected`, `Sold`, `Rerolled`, `CombatStart`, `CombatEnd`, `RunEnded` |
| `timestamp_ms` | `INTEGER NOT NULL` | When the Event was made |
| `day`, `hour` | `INTEGER` | The game's day and hour |
| `details` | `TEXT NOT NULL` | The `EventKind` as JSON, `type` tag included (the id, run, time, day and hour are columns, not in here) |
| `upload_status` | `INTEGER NOT NULL DEFAULT 0` | 0 waiting, 1 sent, 3 rejected (migration 2) |

Indexes `run_tracker_events_by_run` on `(run_id, timestamp_ms, seq)`, and `run_tracker_events_waiting_for_upload` on
`(seq) WHERE upload_status = 0`.

### `imported_shop_odds`

Bazaar DB's Shop odds, one row for each day (migration 3; renamed in 8).

| Column | Type | What it holds |
|---|---|---|
| `day` | `INTEGER PRIMARY KEY` | The day |
| `samples` | `INTEGER NOT NULL` | How many shops the odds come from |
| `bronze`, `silver`, `gold`, `diamond` | `REAL NOT NULL` | The chance of each tier |
| `imported_ms` | `INTEGER NOT NULL` | When they were saved (`saved_ms` before migration 8) |
| `source` | `TEXT NOT NULL DEFAULT 'bazaardb'` | `bazaardb` when fetched, `builtin` when seeded from the defaults |

### `imported_item_odds`

Bazaar DB's odds for single items (migration 4; renamed in 8). Primary key `(template_id, hero, tier)`.

| Column | Type | What it holds |
|---|---|---|
| `template_id` | `TEXT NOT NULL` | The card's template id, lower-cased on save |
| `hero` | `TEXT NOT NULL` | The hero the odds are for |
| `tier` | `TEXT NOT NULL` | The tier the player owns the item at, or `None` (`NOT_OWNED` in `apps/tracker/src/tracking/item_odds.rs`) when not owned |
| `url_id` | `TEXT NOT NULL` | The item's Bazaar DB id, so a refresh can name it |
| `item_name` | `TEXT NOT NULL` | The item's name |
| `body` | `TEXT NOT NULL` | Bazaar DB's answer as it came; read with `bazaardb::item_odds::parse_json` |
| `imported_ms` | `INTEGER NOT NULL` | When they were saved (`saved_ms` before migration 8) |
| `source` | `TEXT NOT NULL DEFAULT 'bazaardb'` | `bazaardb` when asked for in the window, `scraper` when brought in by Import item odds… |

### `run_tracker_tracked_items` and `run_tracker_hidden_items`

The items the player tracks in a Run, to see their odds beside each merchant (migration 5), and the
items they hid from those odds (migration 6). Both have primary key `(run_id, template_id)`.

| Column | Type | What it holds |
|---|---|---|
| `run_id` | `TEXT NOT NULL` | `REFERENCES run_tracker_runs(id) ON DELETE CASCADE ON UPDATE CASCADE` |
| `template_id` | `TEXT NOT NULL` | The card's template id, lower-cased |
| `added_ms` / `hidden_ms` | `INTEGER NOT NULL` | When it was tracked / hidden |

### `setting_pinned_items`

The items the player pins for a hero (migration 7): the app tracks them in each of that hero's Runs,
on top of the Run's own `run_tracker_tracked_items`. Primary key `(hero, template_id)`. They belong to the hero,
not a Run, so deleting or rebinding a Run leaves them; `clear` deletes them with everything else.

| Column | Type | What it holds |
|---|---|---|
| `hero` | `TEXT NOT NULL` | The hero as the game names it, such as `Jules` |
| `template_id` | `TEXT NOT NULL` | The card's template id, lower-cased |
| `pinned_ms` | `INTEGER NOT NULL` | When it was pinned |

## Migrations and `user_version`

The schema version is SQLite's `PRAGMA user_version`. `MIGRATIONS` is a list of SQL batches;
migration N takes a file from version N-1 to N, and `CURRENT_VERSION` is the list's length (8).

| # | Adds |
|---|---|
| 1 | `runs`, `events`, `events_by_run`, `runs_by_start` |
| 2 | `events.upload_status` (default 0) and the partial index `events_waiting_for_upload` |
| 3 | `shop_odds` |
| 4 | `item_odds` |
| 5 | `tracked_items` |
| 6 | `hidden_items` |
| 7 | `pinned_items` |
| 8 | Prefixes every table by where its rows come from ([2026-11-08.002](../records/2026-11-08.002-db-table-prefixes.html)): `run_tracker_runs`, `run_tracker_events`, `run_tracker_tracked_items`, `run_tracker_hidden_items`, `imported_shop_odds`, `imported_item_odds`, `setting_pinned_items`. The three indexes are made again as `run_tracker_runs_by_start`, `run_tracker_events_by_run`, `run_tracker_events_waiting_for_upload`. The odds tables' `saved_ms` becomes `imported_ms`, and they gain `source` (default `bazaardb`) |

The names before migration 8 are kept in the table above as each migration wrote them.

`History::open_with_migrations(path, migrations)` reads `user_version`, and if the file has a
version (above 0) that is behind `migrations`, copies it first with `VACUUM INTO` to
`history.v<version>.bak.db` beside it (`history.v<version>.bak.2.db`, `.bak.3.db`, … when that name
is taken). The copy is the file as it was, at its old version; the app never removes it. Then
`migrate_with(connection, migrations)` reads `user_version` again and:

- **A new file** (version 0) runs every migration. It is not backed up.
- **An older file** runs only the migrations it lacks, in order, each once. Each runs in its own
  transaction together with `PRAGMA user_version = N`, so a failing migration leaves the file at
  the version it had.
- **A newer file** (version above the list's length) is refused with
  `Error::NewerSchema { found, supported }`: "history.db has schema version 9, but this app only
  understands up to 8".

To go back to an older build, close the app and put the backup in place of `history.db` (and
delete any `history.db-wal` and `history.db-shm` beside it).

New migrations go at the end; one that has shipped is never changed.
`History::open_with_migrations(path, migrations)` takes a chosen list, which the tests use.

## Saving an Event

`insert_event(&event)` does everything in one transaction on the writer:

1. `INSERT OR IGNORE` the Run with `id = event.run_id` and `started_ms = event.timestamp_ms`, so
   the first Event of an unknown Run creates it.
2. `INSERT OR IGNORE` the Event, with `details = serde_json::to_string(&event.kind)`.
3. If the Event was new, fold it into the Run's summary.

It returns whether the Event was new. Saving the same Event id again changes nothing and returns
`false`, so a save the tracker retries cannot double up.

How each Event updates the `run_tracker_runs` row:

| Event | Change to the Run |
|---|---|
| Any with a `day` | `day = MAX(COALESCE(day, 0), event.day)`: the day only grows |
| `RunStarted` | `started_ms` = the Event's time; `ended = 0`, `ended_ms` and `outcome` cleared (a Run started again, as when the tracker restarts mid-run, is not ended); `hero` set if the Event has one |
| `CombatEnd` | `victories` and `losses` from `victories_after` and `losses_after`, where present |
| `RunEnded` | `ended = 1`, `ended_ms` = the Event's time, `outcome`, and `victories` and `losses` where present |
| `OptionsPresented`, `Selected`, `Sold`, `CombatStart` | The Snapshot's hero (the player's, for CombatStart) fills `hero` if it is still `NULL` |

`set_run_ended(id, ended)` marks a Run ended (`ended = 1`) or not ended (`ended = 0`, `ended_ms`
and `outcome` cleared), and returns whether the Run exists.

## Reading

| Method | Returns |
|---|---|
| `list_runs()` | Every Run, newest first (`ORDER BY started_ms DESC, rowid DESC`) |
| `get_run(id)` | One Run, or `None` |
| `list_events(run_id)` | The Run's Events in the order they happened (`ORDER BY timestamp_ms, seq`) |
| `load_event(id)` | One Event, or `None` |
| `schema_version()` | The file's `user_version` |

An Event is rebuilt from its columns and `details`; `details` that is not valid JSON is
`Error::Json`.

## Rebinding a Run's id

`rebind_run(old_id, new_id)` gives a `local-` Run its real id, in one transaction. It returns
whether `old_id` existed; the same id twice, or an `old_id` that is not there, does nothing.

- **No Run has `new_id`:** `UPDATE run_tracker_runs SET id = new_id`. The Events, tracked items and hidden
  items follow through `ON UPDATE CASCADE`.
- **A Run with `new_id` exists:** the two merge into it. The Events move to `new_id`; tracked and
  hidden items move with `UPDATE OR IGNORE` (an item `new_id` already has stays under the old id and
  is deleted with the old Run); the merged Run keeps the earlier `started_ms`; the old Run is deleted. The rest
  of the merged Run's summary is the existing Run's.

The tracker calls it when the run state machine produces `Output::Rebound` (see
[run-state-machine.md](run-state-machine.md#rebound)).

## Deleting

`delete_run(id)` deletes the `run_tracker_runs` row and returns whether there was one. `ON DELETE CASCADE`
removes its Events, tracked items and hidden items with it; other Runs are untouched. The window
never deletes the Live run, since the tracker is still saving to it (`apps/tracker/src/tracking/session.rs`).
The odds tables and `setting_pinned_items` do not belong to a Run and are never deleted with one.

## Uploads

Migration 2's `upload_status` records what became of each Event in the upload (see
[upload.md](upload.md)):

| `UploadStatus` | Code | Meaning |
|---|---|---|
| `Waiting` | 0 | Not uploaded yet (any unknown code reads as this) |
| `Sent` | 1 | The server accepted it |
| `Rejected` | 3 | The server refused it for good; it is never tried again |

`waiting_events(limit, stale_before_ms)` returns at most `limit` Events, oldest `seq` first, each
with its JSON, that are waiting, belong to a Run with a real id (not `local-%`), and whose Run has
ended or has had no Event since `stale_before_ms`. The uploader passes now minus its `stale_after`
(6 hours). `set_upload_status(ids, status)` sets the status of many Events in one transaction;
`upload_status(id)` reads one.

## Shop odds and item odds

`imported_shop_odds` holds one copy. `save_shop_odds(odds, now_ms)` deletes every row and inserts
the new days with `source = 'bazaardb'`, in one transaction. `seed_shop_odds(odds, now_ms)` saves
only into an empty table, with `source = 'builtin'`; the window calls it at start with
`bazaardb::shop_odds::defaults()`, so a new file has odds offline. `shop_odds()` returns the days in
order, with `saved_ms` the latest row's `imported_ms` (`None` when empty). Rows seeded before
migration 8 read `bazaardb` (the column's default) until the next Refresh replaces them.

`save_item_odds(&SavedItemOdds)` replaces the row for the same template id, hero and tier, so the
same item for another hero or tier is kept, with `source = 'bazaardb'`. `import_item_odds` does the
same for many with `source = 'scraper'`, replacing a row only when it is newer. Only odds the player
asked for or imported are saved. `item_odds()` returns every row, ordered by template id, hero and
tier. The Rust field stays `SavedItemOdds::saved_ms`; it is the column `imported_ms`. See
[bazaar-db.md](bazaar-db.md) for where the odds come from.

`track_item` and `hide_item` insert with `INSERT OR IGNORE` (doing it again changes nothing);
`untrack_item` and `unhide_item` delete one row. All four lower-case the template id.
`tracked_items(run_id)` and `hidden_items(run_id)` return the ids in the order they were added
(`ORDER BY added_ms, template_id` / `hidden_ms, template_id`).

## Inspecting `history.db`

Open it with the `sqlite3` shell. Use `-readonly` while the app or `just tracker track` is running, so
nothing you type can write to it:

```powershell
sqlite3 -readonly .bazaar-tracker\history.db
```

```sql
-- The schema version (8 today) and the tables.
PRAGMA user_version;
.tables

-- The ten newest Runs, with local times.
SELECT id, hero, day, victories, losses, outcome, ended,
       datetime(started_ms / 1000, 'unixepoch', 'localtime') AS started,
       datetime(ended_ms / 1000, 'unixepoch', 'localtime')   AS ended_at
FROM run_tracker_runs ORDER BY started_ms DESC LIMIT 10;

-- Runs that have not ended (the Live run, or ones the tracker lost), and Runs still under a local id.
SELECT id, hero, day FROM run_tracker_runs WHERE ended = 0;
SELECT id FROM run_tracker_runs WHERE id LIKE 'local-%';

-- How many Events of each kind the newest Run has.
SELECT kind, COUNT(*) FROM run_tracker_events
WHERE run_id = (SELECT id FROM run_tracker_runs ORDER BY started_ms DESC LIMIT 1)
GROUP BY kind ORDER BY 2 DESC;

-- One Run's Events in order, as the app reads them.
SELECT seq, kind, day, hour, details FROM run_tracker_events
WHERE run_id = '3f2a9c1e-...' ORDER BY timestamp_ms, seq;

-- Every item selected in a Run, with its tier and what it merged into.
SELECT day, hour,
       json_extract(details, '$.instance_id') AS instance_id,
       json_extract(details, '$.template_id') AS template_id,
       json_extract(details, '$.tier')        AS tier,
       json_extract(details, '$.merged_into') AS merged_into
FROM run_tracker_events WHERE run_id = '3f2a9c1e-...' AND kind = 'Selected' ORDER BY timestamp_ms, seq;

-- Every combat's result, with its CombatStart, and why the simulation was not used.
SELECT e.day, json_extract(s.details, '$.is_pvp') AS pvp,
       json_extract(s.details, '$.opponent_name') AS opponent,
       json_extract(e.details, '$.outcome') AS outcome,
       json_extract(e.details, '$.sim_diag') AS sim_diag
FROM run_tracker_events e
LEFT JOIN run_tracker_events s ON s.id = json_extract(e.details, '$.combat_start_event_id')
WHERE e.kind = 'CombatEnd' ORDER BY e.timestamp_ms DESC LIMIT 20;

-- The upload backlog by status (0 waiting, 1 sent, 3 rejected).
SELECT upload_status, COUNT(*) FROM run_tracker_events GROUP BY upload_status;

-- The saved Shop odds and item odds.
SELECT day, samples, bronze, silver, gold, diamond, source, datetime(imported_ms / 1000, 'unixepoch') FROM imported_shop_odds;
SELECT template_id, hero, tier, item_name, source, length(body) FROM imported_item_odds;

-- What is tracked and hidden in the newest Run.
SELECT 'tracked', template_id FROM run_tracker_tracked_items WHERE run_id = (SELECT id FROM run_tracker_runs ORDER BY started_ms DESC LIMIT 1)
UNION ALL
SELECT 'hidden', template_id FROM run_tracker_hidden_items WHERE run_id = (SELECT id FROM run_tracker_runs ORDER BY started_ms DESC LIMIT 1);

-- Events whose Run is missing (should be none: foreign keys are on).
PRAGMA foreign_key_check;
```

## Tests

| Test file | Covers |
|---|---|
| `history/history_tests.rs` | A new file at `CURRENT_VERSION` with the prefixed tables and foreign keys on, not backed up; a file at version 7 migrated to 8 with every row, index and cascade kept, and backed up first as `history.v7.bak.db` (then `.bak.2.db`); the odds' `source`; reopening, Events loaded back equal, duplicate saves ignored, time and insertion order, Runs newest first, the Run summary following its Events, the hero from a Snapshot, unknown Runs created, a Run started again no longer ended, `set_run_ended`, deletion, rebinding and merging, each migration run once, a failing migration rolled back, a newer file refused, reads alongside writes, writes from several threads |
| `history/shop_odds_tests.rs` | No odds in a new file, seeding only once, saving replaces every day |
| `history/item_odds_tests.rs` | No odds in a new file, ids lower-cased, saving again replaces only the same hero and tier |
| `history/tracked_items_tests.rs` | Order added, untracking one item, following a rebound Run and going with a deleted one |
| `history/hidden_items_tests.rs` | Hiding and showing again, following a rebound Run and going with a deleted one |
| `history/pinned_items_tests.rs` | Order pinned, by hero, unpinning one item for one hero, staying when a Run is deleted |
| `tracker/tracker_tests.rs` | A recorded log saved through the tracker into a real `history.db`, and a rebinding applied to it |

Run them with `just tracker test`, or `cargo test -p bazaar-tracker history::`.
