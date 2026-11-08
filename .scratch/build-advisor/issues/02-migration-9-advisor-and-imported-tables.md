# 02: Migration 9: advisor_* and imported_* tables in history.db

**What to build:** Add migration 9 to `crates/bazaar-tracker`'s history store. It creates `advisor_conversations`, `advisor_messages`, `imported_runs`, `imported_run_cards`, `imported_picks`, `imported_combats`, `imported_pair_stats`, `imported_pick_stats` and `imported_adjacency_stats`, with the foreign keys and cascades from the spec's history.db decisions. Deleting a Run then removes its Conversations through the cascade.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] Migration 9 creates the tables and their indexes: conversations by run and created time, messages by conversation and seq, imported rows by run, and stats by hero, season and a.
- [ ] `advisor_conversations.run_id` → `run_tracker_runs(id)` `ON UPDATE CASCADE ON DELETE CASCADE`. `advisor_messages.conversation_id` → `advisor_conversations(id)` `ON DELETE CASCADE`. The imported child tables → `imported_runs(run_id)` `ON DELETE CASCADE`.
- [ ] A version-8 file is backed up as `history.v8.bak.db`, then migrated with every row kept.
- [ ] Tests: a new file at version 9; the 8→9 migration; rebinding a Run moves its Conversations; deleting a Run removes its Conversations and their messages.
- [ ] The app's Run delete confirmation says the Run's Conversations go too.
- [ ] Documents: `history-db.md` (the tables, migration 9, and who writes what: the Advisor writes `advisor_*` and the new `imported_*` tables), `db-schema.html` (version 9, ER diagram, SQL), and the `INDEX.md` summaries.

## Comments
