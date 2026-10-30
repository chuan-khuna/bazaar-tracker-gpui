# Bazaar Tracker — Reverse Engineering Index

These are static reverse-engineering notes on BazaarDBTracker 1.0.0-beta.7, the "Bazaar DB" companion app for The Bazaar. It is a .NET 10 / Avalonia app with root namespace `BazaarDBCompanion`. The notes support a rebuild in Rust with GPUI. Each doc marks its claims as verified from code or inferred, and cites the type and member names involved.

- [Overview](overview.md) — what the app does, the tech stack, types grouped by subsystem, the startup flow, and its external surfaces: memory, files, SQLite, network, and the update feed.
- [Submission API](submission-api.md) — the batch upload: gzipped PascalCase JSON POSTed with a Bearer key, 413 split and dead-letter, 15 s to 1 h backoff, DPAPI-protected secret, env URL override.
- [Persistence](persistence.md) — the history.db schema at user_version 6, every SQL statement mapped to its method, locking and transactions, the local-/rebind run lifecycle, batch sync states, and a rusqlite layout.
- [UI](ui.md) — the window tree with an ASCII mockup, view-model bindings and commands, the Bdb* palette and tier colors, styles, how cards, combat nodes and CardCatalog are built, and GPUI mapping notes.
- [App shell](app-shell.md) — startup, the single-instance mutex, the admin relaunch, settings.json, the Velopack update flow, FileLog rotation, and Rust porting notes.
- [Log and events](log-and-events.md) — how Player.log is tailed and parsed, all 7 regexes verbatim, the GameTracker run state machine with its timers, event JSON fields, and which data the log alone can provide.
- [Memory reading](memory-reading.md) — internals of the UnitySpy fork, the mono offset table, field paths for every value read, the collection and JSON readers, connection cadence, GameEnums, and Rust port notes.
