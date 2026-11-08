# 02: Migration 9: advisor_* and imported_* tables in history.db

**What to build:** Add migration 9 to `crates/bazaar-tracker`'s history store. It creates `advisor_conversations`, `advisor_messages`, `imported_runs`, `imported_run_cards`, `imported_picks`, `imported_combats`, `imported_pair_stats`, `imported_pick_stats` and `imported_adjacency_stats`, with the foreign keys and cascades from the spec's history.db decisions. Deleting a Run then removes its Conversations through the cascade.

**Blocked by:** 01

**Status:** resolved

- [x] Migration 9 creates the tables and their indexes: conversations by run and created time, messages by conversation and seq, imported rows by run, and stats by hero, season and a.
- [x] `advisor_conversations.run_id` → `run_tracker_runs(id)` `ON UPDATE CASCADE ON DELETE CASCADE`. `advisor_messages.conversation_id` → `advisor_conversations(id)` `ON DELETE CASCADE`. The imported child tables → `imported_runs(run_id)` `ON DELETE CASCADE`.
- [x] A version-8 file is backed up as `history.v8.bak.db`, then migrated with every row kept.
- [x] Tests: a new file at version 9; the 8→9 migration; rebinding a Run moves its Conversations; deleting a Run removes its Conversations and their messages.
- [x] The app's Run delete confirmation says the Run's Conversations go too.
- [x] Documents: `history-db.md` (the tables, migration 9, and who writes what: the Advisor writes `advisor_*` and the new `imported_*` tables), `db-schema.html` (version 9, ER diagram, SQL), and the `INDEX.md` summaries.

## Comments

Done (not committed). Migration 9 is in `crates/bazaar-tracker/src/history/schema.rs`, with the columns from the record's Database section.

- Indexes: `advisor_conversations_by_run (run_id, created_ms)`, `imported_run_cards_by_run`, `imported_picks_by_run` and `imported_combats_by_run` (the last two on `run_id, day, hour`), and `imported_runs_by_hero (hero, season)`, added for the season sample. For `advisor_messages` (conversation, seq) and the three `_stats` tables (hero, season, a, b) the primary key is the index, so no separate index was made.
- Choices the spec left open: `advisor_messages` has primary key `(conversation_id, seq)`. The `_stats` tables have primary key `(hero, season, a, b)`, so importing twice can't double rows. `advisor_messages` and the imported child tables also cascade on update. `message` and `stats` are JSON in `TEXT`, and `taken` is `INTEGER` 0/1.
- `rebind_run`'s merge path now moves Conversations to the surviving Run. Without this, deleting the old Run would have cascaded them away. The rename path moves them by `ON UPDATE CASCADE`.
- `clear()` now empties the `advisor_*` tables and the new `imported_*` tables too. The Settings Clear data warning names Conversations and Scraped runs.
- Run delete confirmation: while the red `Delete?` waits for its second click, the line under the hero reads "Its Events and Conversations go too" in the error colour. The app has no tooltip widget, so the note goes in the row.
- Tests are in `history_tests.rs`: a new file at version 9 with all 16 tables; 8→9 with every row kept, the new indexes and `history.v8.bak.db` at version 8; a Conversation needs a Run; rebinding (rename and merge) moves Conversations; deleting a Run removes its Conversations and messages; deleting a Conversation removes its messages; deleting a Scraped run removes its cards, picks and combats; `clear` empties the new tables. The old migration-8 test now opens with `MIGRATIONS[..8]`.
- Documents: `history-db.md`, `db-schema.html` and `INDEX.md`, plus one line each in `gpui-app.md` and `README.html` about what deleting a Run removes.
- By-hand check still to do: in the app, hover a past Run, click the trash can and read the note under the hero.
