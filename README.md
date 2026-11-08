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
- [Rust](https://rustup.rs/), stable, 1.92 or later (edition 2024; built and tested with 1.99.1)
- [`just`](https://github.com/casey/just) (optional; each recipe is a plain `cargo` command)

## `just` commands

| Command | What it does | Plain `cargo` |
|---|---|---|
| `just` | Lists the recipes: `tracker` (the gpui app, `just/tracker.just`), `scraper` (the Python odds scraper, `just/scraper.just`) and `advisor` (the Build Advisor, `just/advisor.just`). | — |
| `just tracker run` | Builds and runs the app. A debug build keeps its data in `.bazaar-tracker/` in the repo. | `cargo run -p bazaar-tracker-gpui` |
| `just tracker test` | Runs the tests of every crate in the workspace. | `cargo test --workspace` |
| `just tracker build` | Builds the app in release mode, into `target/release/`. `just tracker release` does the same. | `cargo build --release -p bazaar-tracker-gpui` |
| `just tracker start` | Starts the release build from `just tracker build` without rebuilding, with its data in `.bazaar-tracker/` like `just tracker run`. | `target\release\bazaar-tracker-gpui.exe` with `BAZAAR_TRACKER_DATA_DIR` set |
| `just tracker clean` | Deletes Cargo's build output in `target/`. The next build starts from scratch. | `cargo clean` |
| `just tracker spike` | Memory-reader check: attaches to the running game, prints the Unity version, the offset table, the loaded assemblies and the fields of the four game classes the tracker uses, then reads the `BoardManager` singleton and the entity count. Use it first after a game update. | `cargo run -p unityspy-rs --example dump-classes` |
| `just tracker snapshot` | Prints one Snapshot of the running game: run id, hero, run state, stats, board, stash, skills, opponent, rank and last combat. Add `-- --watch` to print one every poll. | `cargo run -p bazaar-tracker --example snapshot` |
| `just tracker track` | Runs the whole tracker without a window. It follows the live game, saves to `history.db` and prints each Event as it is saved. Add `-- --log <path>` to follow another `Player.log`. Stop it with Ctrl+C. | `cargo run -p bazaar-tracker --example track` |
| `just scraper sync` | Installs the odds scraper's Python dependencies (uv) into `apps/scraper/.venv`. | `cd apps/scraper && uv sync` |
| `just scraper list` | Prints what a scrape would ask Bazaar DB for, from the `items.json` the app writes. | `uv run scrape-odds --list` |
| `just scraper scrape` | Saves item odds to `.bazaar-tracker/raw_data/scraper/odds/<template_id>.json`. Each item's odds are saved at every owned tier it has, clicked through on one page. With no arguments it asks which characters, whether to skip what is already scraped, whether to retry the skipped list, and how many retries, shows the queue, and logs each item in colour under a progress bar; with arguments (`just scraper help`) it runs straight away. Items that get no odds (no match in the page's search) are kept in `.bazaar-tracker/raw_data/scraper/odds/skipped.json` and left out of later runs; `--retry-skipped` asks for them again. Bazaar DB's Cloudflare check usually blocks the browser it launches. Add `--cdp http://localhost:9222` to scrape in a new tab of a browser you started with `--remote-debugging-port=9222` (such as Dia), with that browser's own profile and Cloudflare clearance. | `uv run scrape-odds` |
| `just advisor sync` | Installs the Build Advisor's Python dependencies (uv) into `apps/advisor/.venv`. | `cd apps/advisor && uv sync` |
| `just advisor run -m "…"` | Asks the Advisor one question in the terminal. Add `--situation file.json` for a situation, or `--fake` to check it without a model or key. Set the model endpoint and key in `apps/advisor/.env` (see [Advisor](docs/documents/advisor.md)). | `uv run advisor -m "…"` |
| `just advisor serve` | Runs the Advisor's service for the tracker's Advisor tab on `127.0.0.1:8420`. Add `--fake` to answer without a model or key. Stop it with Ctrl+C. | `uv run advisor-serve` |
| `just advisor test` | Runs the Advisor's tests. | `uv run pytest` |

`spike`, `snapshot` and `track` need the game running. Don't run `just tracker track` and the app at the same time: both write to the same `history.db`.

## Where data lives

| Build | Folder |
|---|---|
| Debug (`just tracker run`) | `<repo>/.bazaar-tracker/` (gitignored) |
| Release | `%APPDATA%\BazaarTracker\` |
| Either, overridden | `BAZAAR_TRACKER_DATA_DIR` |

When you build from the repo, keep the release build's data in `.bazaar-tracker/` too: `just tracker start` sets `BAZAAR_TRACKER_DATA_DIR` for you. See [Data folder](docs/documents/data-folder.md) for setting the variable yourself.

The folder holds:

- `settings.toml`: the theme, the `Player.log` path, `run_sync` and the window layout;
- `history.db`: your Runs and Events;
- `logs/tracker.log`.

The Bazaar DB secret key is kept in the Windows credential store, not in a file.

## Repository layout

```
Cargo.toml            the workspace (virtual): apps/tracker and crates/*
apps/
  tracker/            the GPUI app (its Cargo.toml, macos/ signing scripts and src/)
    src/
      shell/          single-instance lock, settings, data folder, logging
      overlays/       menu, palette, theme picker and settings state (no GPUI)
      theme/          the ten themes and their colour roles
      ui/             the views: title bar, sidebar, main panel, detail, log panel, status bar
  scraper/            the Python scrapers (item odds, runs) from Bazaar DB
  advisor/            the Build Advisor (Python): an agent that answers questions about the Run being played
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
- Memory reading depends on the game's Unity version and class layout. A game update can break it; `just tracker spike` shows what changed.
