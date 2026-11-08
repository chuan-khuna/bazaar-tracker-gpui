# 01: Move the repo to apps/tracker, apps/scraper, apps/advisor

**What to build:** Move the gpui app from the root to `apps/tracker` and `app/scraper` to `apps/scraper`, make the root `Cargo.toml` a virtual workspace, and rename `just app` to `just tracker`. Make no change in behaviour. See the spec's Repo layout decisions.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] `git mv` the root `src/` and `macos/` into `apps/tracker/`, with the package part of the root `Cargo.toml` becoming `apps/tracker/Cargo.toml`. `git mv app/scraper apps/scraper`, recreating its `.venv` rather than moving it if moving breaks it.
- [ ] The root `Cargo.toml` has only `[workspace]` with `members = ["apps/tracker", "crates/*"]`, plus the shared settings it had. `Cargo.lock` and `target/` stay at the root.
- [ ] `shell::data_dir` still gives `<repo>/.bazaar-tracker/` in a debug build (it now goes up from `apps/tracker`), with a test.
- [ ] `just/app.just` becomes `just/tracker.just`, imported as `tracker`. `just/scraper.just` points at `../apps/scraper`. Every recipe still works (`just tracker run/build/start/test`, `just scraper …`).
- [ ] `macos/run.sh` and `sign.sh`, `mise.toml`, `.gitignore`, `skills-lock.json` if it names paths, and any CI paths are updated.
- [ ] `cargo build`, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean.
- [ ] Documents updated for the new paths and `just tracker`: `architecture.md` (repo layout), `modules.md`, `gpui-app.md`, `data-folder.md`, `scrapers.md`, `scraping-manual.html`, `odds-scraper.md`, `README.html`, `INDEX.md`, the root `README.md`, and `CLAUDE.md` if it names paths. Records are left as written.

## Comments
