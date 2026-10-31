# Bazaar Tracker

A desktop companion for [The Bazaar](https://playthebazaar.com/). It follows the game while you play and keeps a history of each run: the options you were offered, what you picked, bought and sold, your board at every step, and how each fight ended.

It is a Rust rewrite of BazaarDBTracker, with a native window built on [GPUI](https://www.gpui.rs/). It is its own product: it reads and writes nothing of the original tracker's.

> **Status:** early. Windows only. The app tracks a live game and shows your runs, but it has only just been tried against the real game. See the records in `docs/records/` for what has been checked.

## How it works

The tracker combines two sources:

- **`Player.log`.** The game's own log file, followed as it grows. It tells the tracker when a run starts and ends, and when you are offered options, pick one, sell an item or enter a fight.
- **Game memory.** The tracker attaches to the running game read-only and reads its Mono objects: the real run id, your board, stash and skills, gold and health, the shop's items, the opponent, rank and combat results.

A run state machine turns both into **Events** and saves them, with their Runs, to `history.db`. Card names come from the game's own card catalog.

## The app

- **Sidebar:** your runs, newest first. A live run is marked LIVE.
- **Main panel:**
  - **Live:** what the game shows now, updated about once a second. In a shop that is the shop's items and your items; at an encounter choice, the options.
  - **Snapshot:** your board, stash and skills at the selected moment. Pick an event or a day button to look back.
- **Detail panel:** the selected event, such as the options with your pick ticked or the boards of a fight.
- **Log panel:** the last 2,000 log rows as plain sentences, with process and memory details pinned at the top.
- **Status bar:** whether the game is attached and being tracked.
- **Command palette and themes:** a command palette and ten themes; Rosé Pine is the default.

Uploading runs to Bazaar DB is built but **off by default** (`run_sync` in `settings.toml`).

## Requirements

- Windows 10 or 11, x64
- The Bazaar (Steam)
- [Rust](https://rustup.rs/), stable, edition 2024
- [`just`](https://github.com/casey/just) (optional; each recipe is a plain `cargo` command)

## `just` commands

| Command | What it does | Plain `cargo` |
|---|---|---|
| `just` | Lists the recipes. | — |
| `just run` | Builds and runs the app. A debug build keeps its data in `.bazaar-tracker/` in the repo. | `cargo run` |
| `just test` | Runs the tests of every crate in the workspace. | `cargo test --workspace` |
| `just release` | Builds the app in release mode, into `target/release/`. | `cargo build --release` |
| `just spike` | Memory-reader check: attaches to the running game, prints the Unity version, the offset table, the loaded assemblies and the fields of the four game classes the tracker uses, then reads the `BoardManager` singleton and the entity count. Use it first after a game update. | `cargo run -p unityspy-rs --example dump-classes` |
| `just snapshot` | Prints one Snapshot of the running game: run id, hero, run state, stats, board, stash, skills, opponent, rank and last combat. Add `-- --watch` to print one every poll. | `cargo run -p bazaar-tracker --example snapshot` |
| `just track` | Runs the whole tracker without a window. It follows the live game, saves to `history.db` and prints each Event as it is saved. Add `-- --log <path>` to follow another `Player.log`. Stop it with Ctrl+C. | `cargo run -p bazaar-tracker --example track` |

`spike`, `snapshot` and `track` need the game running. Don't run `just track` and the app at the same time: both write to the same `history.db`.

## Where data lives

| Build | Folder |
|---|---|
| Debug (`just run`) | `<repo>/.bazaar-tracker/` (gitignored) |
| Release | `%APPDATA%\BazaarTracker\` |
| Either, overridden | `BAZAAR_TRACKER_DATA_DIR` |

The folder holds:

- `settings.toml`: the theme, the `Player.log` path, `run_sync` and the window layout;
- `history.db`: your Runs and Events;
- `logs/tracker.log`.

The Bazaar DB secret key is kept in the Windows credential store, not in a file.

## Repository layout

```
src/                  the GPUI app
  shell/              single-instance lock, settings, data folder, logging
  overlays/           menu, palette, theme picker and settings state (no GPUI)
  theme/              the ten themes and their colour roles
  ui/                 the views: title bar, sidebar, main panel, detail, log panel, status bar
crates/
  unityspy-rs/        reads a Unity game's Mono memory (a corrected Rust port of UnitySpy)
  bazaar-tracker/     the Bazaar-specific tracker: log, memory, run state machine,
                      history, card catalog, upload; no UI
docs/
  documents/          reference documents (start with architecture.md)
  records/            dated records of discussions and of runs against the game
  rea/                reverse-engineering notes on the original tracker and UnitySpy
  agents/             how agents use the issue tracker, labels and domain docs
.scratch/             specs and tickets
GLOSSARY.md           the domain terms (Run, Event, Snapshot, ...)
```

## Notes

- The tracker only **reads** the game's memory. It never writes to it or injects anything.
- Memory reading depends on the game's Unity version and class layout. A game update can break it; `just spike` shows what changed.
