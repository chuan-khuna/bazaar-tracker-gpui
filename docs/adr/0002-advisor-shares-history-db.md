# 2. The Advisor shares history.db, and the tracker app owns its schema

Date: 2026-11-08

## Status

Accepted

## Context

The Advisor ([2026-11-08.001](../records/2026-11-08.001-build-advisor-agent.html)) is a Python
service (FastAPI, built like tau-app) beside the Rust tracker app. It keeps Conversations and their
messages, and it imports Scraped runs and statistics. A Conversation belongs to a Run, and a Run's id
can change: it starts as `local-…` and is rebound to its real id later. A Run can also be deleted.

The usual setup would give the Python service its own database, managed with Alembic, as tau-app
does. But then every Conversation would hold a `run_id` that the tracker app could rename or delete
without the service knowing, and the app would have to tell the service about each change.

## Decision

The Advisor's tables live in `history.db`, prefixed `advisor_*` (Conversations, messages) and
`imported_*` (Scraped runs and their stats), as described in
[2026-11-08.002](../records/2026-11-08.002-db-table-prefixes.html).

- `advisor_conversations.run_id` references `run_tracker_runs(id)` `ON UPDATE CASCADE ON DELETE
  CASCADE`, so rebinding and deleting a Run carry its Conversations with no extra code.
- **The tracker app owns the schema.** Every table, the Advisor's included, is created by a Rust
  migration in `crates/bazaar-tracker`'s history store (from migration 9).
- The Advisor uses no Alembic and never migrates. It opens the file with WAL, `foreign_keys=ON` and
  `busy_timeout`, checks that `user_version` is the version it was written for, and refuses
  otherwise.
- The Advisor writes only `advisor_*` and its own `imported_*` tables. It never writes
  `run_tracker_*`.

## Considered Options

- **A separate `advisor.db`, managed by Alembic.** This was rejected because Conversations would
  point at Run ids that can change or disappear, and the app and the service would have to keep them
  in step by hand.
- **A link table in `history.db` with the messages in `advisor.db`.** This handles rebinding through
  a cascade, but it splits one Conversation across two files, and deleting a Run would still need a
  call to the service.
- **Postgres.** This was rejected: it means one more server to start, for a few thousand rows.

## Consequences

- A change to the Advisor's tables needs a Rust migration first. The Python models then follow it,
  and the version the Advisor accepts is raised at the same time.
- Two processes write to one file. WAL and `busy_timeout` cover this, but long Advisor transactions
  (an import) should commit in batches so the tracker's saves are never held up.
- Imported Scraped runs make `history.db` larger, and each migration's backup copies them too.
  If that becomes a problem, the `imported_*` tables can move to their own file. They hold no
  references to Runs, so the move is cheap.
