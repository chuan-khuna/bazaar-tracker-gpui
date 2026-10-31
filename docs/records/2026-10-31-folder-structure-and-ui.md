# Folder structure, theming and UI layout

## What was discussed

Following the [feasibility record](2026-10-30-riir-feasibility.md), we settled how the repo is laid out, how the gpui app is themed and laid out (modelled on `D:\code\wordle`'s `app/wordle-gpui`), where the app keeps its data, and how far the app stays tied to the original BazaarDBTracker. The window is drawn in [2026-10-31-ui-mockup.html](2026-10-31-ui-mockup.html).

## Decided

### Repo layout

- The root `Cargo.toml` is both the gpui app's package (`bazaar-tracker-gpui`, code in `src/`) and the workspace. Library crates live in `crates/<crate-name>/`.
- Folder and package names are kebab-case and identical (`crates/unityspy-rs` → package `unityspy-rs` → `use unityspy_rs`).
- Two library crates:
  - **`unityspy-rs`**: generic Unity Mono memory reading, the corrected port of UnitySpy. Modules `mem/` (process, reader, module) and `mono/` (pe, domain, offsets, class, field, statics, object, value, bcl), as in `docs/rea/unityspy/rust-port.md` §2. The build-step-1 spike is `examples/dump-classes.rs`.
  - **`bazaar-tracker`**: everything Bazaar-specific with no UI. Modules `log/`, `memory/` (the trait the run state machine reads through, and its implementation on `unityspy-rs`), `run/`, `enums/`, `history/`, `catalog/`, `upload/`. Recorded logs in `tests/fixtures/logs/`.
- The app's `src/` follows wordle-gpui's layers: `shell/` (single-instance lock, admin relaunch, settings, logging, update placeholder), `overlays/` (state and keys, no gpui), `theme/`, `ui/`. Tests sit beside the code as `<name>_tests.rs`.
- Windows only for now. `unityspy-rs` is `#![cfg(windows)]`; a log-only macOS build would plug in behind the memory trait later.
- The gpui stack is pinned to wordle's: `gpui-pre =0.3.9` (as `gpui`), `gpui-pre-platform`, `gpui-base =0.7.3`, `gpui-kit-assets`, `gpui-component 0.7.3` without default features. The title bar, menu, palette, search, theme picker and widgets are copied from wordle-gpui, not shared through a crate.
- A root `justfile` with `run`, `test` (`cargo test --workspace`), `spike` and `release`.

### UI

- wordle's frame: a flat title bar with only the title and ☰, island panels, a flat status bar, a ⌘P palette with the same commands as the menu. The runs list is the left sidebar (⌘B), the run is the main island, the event detail is a right island (⌘⌥B).
- The status pills move to the status bar. Errors show there and stay until dismissed. Settings open from a menu row as an overlay.
- The main island shows the **board, stash and skills at the selected moment**: live while following a live run, the last snapshot of a finished run, or the nearest snapshot at or before a selected event. The detail island keeps what is specific to the event (options, opponent board).
- Items sit on a 10-slot grid (Small 1, Medium 2, Large 3) with a tier border. Badges on top for damage, shield, heal, burn, poison, regen, ammo and `x2` multicast; sell price bottom-left; cooldown bottom-right; everything else in the tooltip.
- No card art for now; tiles leave room for it. Revisit after Bazaar DB answers what they allow.

### Theming

- All ten wordle themes ship; **Rosé Pine** is the default and first in `THEMES`.
- `Theme` keeps wordle's generic roles, drops `correct/present/absent/absent_key`, and adds `win, loss, live`, the tiers `bronze, silver, gold, diamond, legendary`, and the badges `damage, shield, heal, burn, poison, regen, ammo, value`.
- The chosen theme is saved in `settings.toml`; an unknown name falls back to Rosé Pine.

### Data and independence from the original

- The app is its own product and does not read, write or import anything of BazaarDBTracker's. This drops `history.db` compatibility (and the .NET timestamp format), the `secret.dat` import and any use of `%APPDATA%\BazaarDBCompanion\`.
- The user-facing name is **Bazaar Tracker**. Release builds keep data in `%APPDATA%\BazaarTracker\`: `settings.toml`, `history.db`, `logs/tracker.log`. Debug builds use `<repo>/.bazaar-tracker/` (gitignored), and `BAZAAR_TRACKER_DATA_DIR` overrides either. The secret key is in the credential store, under a separate name in debug builds.
- `settings.toml` keys: `theme`, `log_path`, `run_sync`, `show_run_duration`, and a `[window]` table (`sidebar_open`, `sidebar_width`, `detail_open`, `stash_open`).
- `history.db` gets our own schema: `runs` and `events` tables with event details as JSON, integer-millisecond timestamps, foreign keys on, run-id rebinding in one transaction, migrations via `user_version`.
- Uploading to Bazaar DB stays in scope as build step 5, as `User-Agent: BazaarTracker/<version>`, off by default until Bazaar DB answers open question 3.
- No auto-update for now; `shell/update.rs` is a placeholder.

## Open questions

1. The stash's real capacity. The socket enum has ten values for every container; check the live game.
2. Whether the live board is read only at events or also polled; start with events.
3. Card art source, after Bazaar DB's answer.
