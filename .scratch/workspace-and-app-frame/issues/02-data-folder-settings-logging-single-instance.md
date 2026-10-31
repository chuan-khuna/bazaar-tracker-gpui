# 02: Data folder, settings, logging and single instance

**What to build:** The app keeps its own data. On launch it finds its data folder (`BAZAAR_TRACKER_DATA_DIR`, else the repo's `.bazaar-tracker/` in a debug build, else `%APPDATA%\BazaarTracker\`), reads `settings.toml` with defaults for anything missing, and writes a rotating `logs/tracker.log`. Launching a second copy brings the first window forward instead. See the spec's Settings, Data folder, Logging and Single instance decisions.

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** resolved

- [x] Data-folder resolution follows the order above; the debug path is fixed at compile time from the manifest directory; the folder is created if missing.
- [x] `settings.toml` has the keys `theme`, `log_path`, `run_sync`, `show_run_duration` and a `[window]` table (`sidebar_open`, `sidebar_width`, `detail_open`, `stash_open`), all optional with the spec's defaults.
- [x] Unknown keys are ignored; an unparsable file is logged, defaults are used in memory, and the file is not overwritten until a setting changes.
- [x] Settings are saved as soon as they change.
- [x] `logs/tracker.log` is written through `tracing`, rotates by size keeping the current file and a few backups, and starts each launch with a banner holding the version and process id.
- [x] A second launch signals the running copy to show and focus its window, then exits.
- [x] Tests (settings and data-folder seam): resolution as a function of env var, build kind and app-data path; empty file gives defaults; partial file keeps given keys; unknown keys ignored; unparsable file gives defaults and is left untouched; save-then-load round-trips.
- [x] A debug `just run` creates `.bazaar-tracker/settings.toml` and `.bazaar-tracker/logs/tracker.log`, both gitignored.
