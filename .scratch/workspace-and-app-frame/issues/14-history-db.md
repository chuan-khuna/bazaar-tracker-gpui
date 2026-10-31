# 14: `history.db`

**What to build:** Runs and Events are saved to `history.db` in the data folder with our own schema, and can be listed, loaded, deleted and rebound. See the spec's History decision. Do not follow `docs/rea/bazaar-tracker/persistence.md` §7's advice to keep the original schema; compatibility was dropped.

**Blocked by:** 10 (Event and Snapshot types, and the game-memory trait)

**Status:** ready-for-agent

- [ ] Tables `runs` and `events`; Event details stored as JSON; timestamps as integer milliseconds since the Unix epoch; foreign keys on.
- [ ] The schema version lives in `user_version`; a fresh file gets the current schema and older versions migrate forward once.
- [ ] Inserting an Event updates its Run's summary (hero, day, record, outcome, started and ended times) in the same transaction.
- [ ] List Runs newest first; list a Run's Events in order; load one Event; delete a Run with its Events; mark a Run ended or not.
- [ ] Rebinding a Run id moves the Run and its Events in one transaction that stays valid with foreign keys on.
- [ ] Writes are serialised; reads can run alongside them.
- [ ] Tests (history seam): against a temporary database, covering each operation above.
