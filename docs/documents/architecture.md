# Architecture

How the repo is laid out and which code may use which. Decided in [2026-10-31-folder-structure-and-ui](../records/2026-10-31-folder-structure-and-ui.md).

## Layout

```
bazaar-tracker-gpui/
  Cargo.toml              [workspace] members = ["apps/tracker", "crates/*"] (virtual: no package)
  Cargo.lock  target/     shared by every member
  justfile                imports just/tracker.just (`just tracker …`), just/scraper.just (`just scraper …`)
                          and just/advisor.just (`just advisor …`)
  apps/
    tracker/              the gpui app
      Cargo.toml            [package] bazaar-tracker-gpui
      macos/                run.sh, sign.sh, entitlements.plist: Cargo's runner on macOS signs each binary
      src/
        main.rs  args.rs
        shell/                app plumbing with no drawing
          instance.rs           single-instance lock
          elevate.rs            relaunch as admin, if it turns out to be needed
          settings.rs           settings.toml
          logging.rs            tracing to logs/tracker.log
          update.rs             placeholder; no auto-update yet
        overlays/             state and keys with no gpui: commands.rs, search.rs, theme_picker.rs, settings_overlay.rs, shop_odds.rs
        tracking/             what the app knows of the tracker, no gpui
          service.rs status.rs session.rs names.rs when.rs   shared: service bridge, connection, runs and selection
          shop_odds.rs          which shop odds show beside the Economy and in the widget
          run/                  view models of the selected Run: header, moment, board, economy, events, detail
          live.rs               the Live section
          log/                  the log panel: details, feed, narrate, rows
        theme/                mod.rs (Theme roles, THEMES) + one file per theme
        ui/                   drawing, in folders named as tracking/'s: mod.rs (TrackerApp) + widgets
          frame/                the window: title_bar, menu, palette, theme_picker, settings_overlay, sidebar, status_bar
          run/                  main_panel, header, economy, board, events, detail
          live.rs  log_panel.rs  shop_odds.rs (the column beside the Economy, and the widget pop-up)
          pop_out.rs            Economy, Shop odds and Item odds stacked in a blurred window over the
                                game, closed while the main window is in front
    scraper/              the Python scrapers (uv): item odds and runs from Bazaar DB, into .bazaar-tracker/raw_data/scraper/
    advisor/              the Build Advisor (Python, uv): an agent on tau_agent/tau_ai that answers questions about
                          the Run being played; shares history.db with the tracker (see advisor.md)
      src/agents/           core/ (from tau-app), common/tools/, advisor/ (the ADVISOR agent)
      src/knowledge/        the numbers, no LLM code: cards.py (GameData.db), the season sample, stats and queries
      src/importer/         Scraped runs into imported_* (`just advisor import`)
      src/backend/          FastAPI on 127.0.0.1:8420 (`just advisor serve`): Conversations, messages, the SSE
                            reply; db/ opens history.db safely, SQLAlchemy models, the Store
      tests/                pytest
  crates/
    unityspy-rs/          generic Unity Mono memory reading (Windows only)
      src/mem/              process.rs, reader.rs (the Memory trait), module.rs
      src/mono/             pe, domain, offsets, class, field, statics, object, value, bcl
      examples/dump-classes.rs
    bazaar-tracker/       everything Bazaar-specific with no UI
      src/log/              following and parsing Player.log
      src/memory/           the trait the run state machine reads through; the real one uses unityspy-rs
      src/run/              the run state machine
      src/enums/            the game's enum values
      src/history/          history.db (runs, events, shop odds)
      src/catalog/          the card catalog
      src/upload/           uploading to Bazaar DB, the secret key
      tests/fixtures/logs/  recorded Player.log files
    bazaardb/             reading Bazaar DB (bazaardb.gg): the shop odds; no UI
      src/fetch.rs          one plain GET, telling a refusal or Cloudflare challenge apart
      src/shop_odds.rs      the /api/shop-odds JSON
      examples/shop_odds.rs `just tracker shop-odds`
```

Folder names equal package names, in kebab-case.

## Layering

- `unityspy-rs` knows nothing about The Bazaar. Its `mono/` modules read only through `mem::Memory`, so tests can feed them byte buffers.
- `bazaar-tracker` knows nothing about gpui. The run state machine reads game state only through `memory/`'s trait, so recorded logs with a fake memory drive its tests.
- In the app, each layer uses only those above it: `shell/`, `overlays/`, `theme/` and `tracking/` have no drawing; `ui/` draws their state and `bazaar-tracker`'s.
- Tests sit beside the code as `<name>_tests.rs`, pulled in with `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;`. Whole-log replays go in `crates/bazaar-tracker/tests/`.
- The root `Cargo.toml` is a virtual workspace, so a plain `cargo test` at the root tests every member; `just tracker test` runs `cargo test --workspace`.
- The three apps share no code. The tracker and the Advisor share `history.db`: the tracker owns its schema and migrates it; the Advisor opens it only at the version it was written for and writes only `advisor_*` and the Scraped run `imported_*` tables ([advisor.md](advisor.md)). In the Advisor, `knowledge/` has no LLM code and `agents/` calls it through tools.

## Data

| Build | Folder |
|---|---|
| Release | `%APPDATA%\BazaarTracker\` |
| Debug | `<repo>/.bazaar-tracker/` (gitignored) |
| Either, overridden | `BAZAAR_TRACKER_DATA_DIR` |

Each holds `settings.toml`, `history.db` and `logs/tracker.log`. The Advisor finds the same folder through `BAZAAR_TRACKER_DATA_DIR`, else `<repo>/.bazaar-tracker/`. The secret key is in the Windows credential store, under a separate name in debug builds. Nothing is shared with the original BazaarDBTracker.

## Theme

Ten themes, one file each in `apps/tracker/src/theme/`; Rosé Pine is the default. The UI uses only the roles:

- from wordle: `background, surface, overlay, highlight_low/med/high, text, subtle, muted, heading, error, success, accent`
- outcome: `win, loss, live`
- tiers: `bronze, silver, gold, diamond, legendary`
- badges: `damage, shield, heal, burn, poison, regen, ammo, value`

The chosen theme is saved as `theme` in `settings.toml`.
