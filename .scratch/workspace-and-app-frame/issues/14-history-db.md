# 14: `history.db`

**What to build:** Runs and Events are saved to `history.db` in the data folder with our own schema, and can be listed, loaded, deleted and rebound. See the spec's History decision. Do not follow `docs/rea/bazaar-tracker/persistence.md` §7's advice to keep the original schema; compatibility was dropped.

**Blocked by:** 10 (Event and Snapshot types, and the game-memory trait)

**Status:** resolved

- [x] Tables `runs` and `events`; Event details stored as JSON; timestamps as integer milliseconds since the Unix epoch; foreign keys on.
- [x] The schema version lives in `user_version`; a fresh file gets the current schema and older versions migrate forward once.
- [x] Inserting an Event updates its Run's summary (hero, day, record, outcome, started and ended times) in the same transaction.
- [x] List Runs newest first; list a Run's Events in order; load one Event; delete a Run with its Events; mark a Run ended or not.
- [x] Rebinding a Run id moves the Run and its Events in one transaction that stays valid with foreign keys on.
- [x] Writes are serialised; reads can run alongside them.
- [x] Tests (history seam): against a temporary database, covering each operation above.

## Comments

Implemented by an agent.

- `history::History` (`History::open(<data folder>/history.db)`, `history::FILE_NAME`): `insert_event`, `list_runs`, `get_run`, `list_events`, `load_event`, `delete_run`, `set_run_ended`, `rebind_run`.
- Schema (migration 1): `runs(id, started_ms, ended_ms, ended, hero, day, victories, losses, outcome)` and `events(seq, id, run_id, kind, timestamp_ms, day, hour, details)` where `details` is the Event kind as JSON (the common fields are columns). `events.run_id` references `runs.id` with `ON DELETE CASCADE ON UPDATE CASCADE`; foreign keys are on for both connections. The schema version is `user_version`; `MIGRATIONS` is an ordered list (ticket 17 appends one), each run once with its version bump in a transaction, and a file from a newer app is refused.
- Rebinding is a single `UPDATE runs SET id` that cascades to the events, so it cannot break the foreign keys the way the original's row-by-row rename did. If the real id already has a Run, the two are merged into it (earlier start kept) and the local one deleted.
- Saving the same Event id twice is ignored (`insert_event` returns `false`), so a retried save after a failure cannot duplicate it.
- Writes use one connection behind a lock and reads another, with the file in WAL mode, so listing does not wait for a write; both are tested with threads.
- Run summary choices where the spec is silent: `day` only grows; `RunStarted` for an existing run un-ends it and sets `started_ms`; the hero comes from `RunStarted` or, if that had none, the first Snapshot that names one; the record comes from `CombatEnd` and `RunEnded`.
