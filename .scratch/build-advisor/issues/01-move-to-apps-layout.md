# 01: Move the repo to apps/tracker, apps/scraper, apps/advisor

**What to build:** Move the gpui app from the root to `apps/tracker` and `app/scraper` to `apps/scraper`, make the root `Cargo.toml` a virtual workspace, and rename `just app` to `just tracker`. Make no change in behaviour. See the spec's Repo layout decisions.

**Blocked by:** none

**Status:** resolved

- [x] `git mv` the root `src/` and `macos/` into `apps/tracker/`, with the package part of the root `Cargo.toml` becoming `apps/tracker/Cargo.toml`. `git mv app/scraper apps/scraper`, recreating its `.venv` rather than moving it if moving breaks it.
- [x] The root `Cargo.toml` has only `[workspace]` with `members = ["apps/tracker", "crates/*"]`, plus the shared settings it had. `Cargo.lock` and `target/` stay at the root.
- [x] `shell::data_dir` still gives `<repo>/.bazaar-tracker/` in a debug build (it now goes up from `apps/tracker`), with a test.
- [x] `just/app.just` becomes `just/tracker.just`, imported as `tracker`. `just/scraper.just` points at `../apps/scraper`. Every recipe still works (`just tracker run/build/start/test`, `just scraper …`).
- [x] `macos/run.sh` and `sign.sh`, `mise.toml`, `.gitignore`, `skills-lock.json` if it names paths, and any CI paths are updated.
- [ ] `cargo build`, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean.
- [x] Documents updated for the new paths and `just tracker`: `architecture.md` (repo layout), `modules.md`, `gpui-app.md`, `data-folder.md`, `scrapers.md`, `scraping-manual.html`, `odds-scraper.md`, `README.html`, `INDEX.md`, the root `README.md`, and `CLAUDE.md` if it names paths. Records are left as written.

## Comments

Done in the working tree, not committed.

- Moves: `src/` and `macos/` to `apps/tracker/` and `app/scraper` to `apps/scraper` with `git mv` (the ignored `.venv/` and `.browser-profile/` went with the folder). The moved `.venv` still worked; `just scraper sync` only reinstalled the project itself from its new path. `apps/tracker/Cargo.toml` is a new file holding the old `[package]` and dependency sections (crate paths now `../../crates/…`), so git shows it as added rather than renamed.
- Root `Cargo.toml`: `[workspace]` with `members = ["apps/tracker", "crates/*"]`, `resolver = "3"` (a virtual workspace has no edition to take it from; edition 2024 already meant 3), and the `[profile.dev.package."*"]` setting, which must live at the workspace root.
- `shell::data_dir`: a `repo_dir()` takes two ancestors of `CARGO_MANIFEST_DIR`; the test `the_debug_folder_is_at_the_repos_root` checks the manifest is `<repo>/apps/tracker` and the root holds `Cargo.lock` and `GLOSSARY.md`.
- `just/tracker.just` (imported as `tracker`): `run` and `build` now pass `-p bazaar-tracker-gpui`, so a virtual workspace builds only the app as before; the macOS `build` signs with `apps/tracker/macos/sign.sh`. `.cargo/config.toml`'s runner is `apps/tracker/macos/run.sh`. `mise.toml`, `.gitignore` and `skills-lock.json` name no moved paths; there is no CI.
- Code comments that named `app/scraper` or the app's `src/…` now name the new paths (the scrapers' Python paths count folders up and are unchanged in depth).
- Docs: the listed documents, `INDEX.md`, the root `README.md`, also `bazaar-db.md`, `card-catalog-and-art.md`, `history-db.md`, `memory-reading.md`, `player-log.md`, `run-state-machine.md`, `upload.md`, `db-schema.html` and ADR 0001's pointer to `when.rs`. The old "a plain `cargo test` at the root tests only the app" is now "tests every member". Records left as written.
- Verified: `cargo build`, `cargo test --workspace` (659 passed), `just tracker test`, `just tracker build` (release), `just -n tracker run` / `start` (paths resolve to the repo root), `just scraper sync`, `help`, `runs --help`, `browser --print`. Not run: GUI and game-bound recipes, `just scraper scrape`/`runs`/`list`/`index`.
- Left unticked: `cargo clippy --workspace --all-targets` gives 6 warnings (`ui/pop_out.rs`, `ui/widgets.rs`, `ui/mod.rs`, `tracking/item_odds_tests.rs`). They are the same on `HEAD` before the move (checked in a scratch worktree), so they were left for a separate change rather than mixed into a pure move.
