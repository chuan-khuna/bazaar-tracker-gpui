# 01: Start the tracker and show the connection

**What to build:** The app depends on `bazaar-tracker`, starts its tracker service on a background thread when it opens, forwards the service's notices into gpui, and the status bar shows the connection state and the errors the service reports. See the spec's Starting the service, Bridge, Connection state and Errors decisions.

**Blocked by:** none

**Status:** ready-for-human

- [x] The root package depends on `bazaar-tracker`; a new plain `src/tracking/` layer (no gpui) holds the bridge and the status logic.
- [x] `main` opens `history.db` in the data folder, resolves `Player.log` from `log_path` and starts `Tracker::start_live`; the app still opens if `history.db` cannot be opened, with a sticky error and "Not tracking".
- [x] One thread owns the `Tracker` and forwards its notices, in order, into an `async_channel`; dropping the service stops it and joins the tracker; `main` drops it when `run` returns.
- [x] `TrackerApp` receives the notices without blocking the window and redraws on each.
- [x] The status bar shows "Tracking" (success dot) when the game is attached, "Waiting for the game" (muted dot) when it is not, and "Not tracking" (muted dot) when the service is not running.
- [x] `SaveFailed`, `LogError` and a failed start show as sticky errors; an error equal to the last one reported is not shown again until an Event is saved.
- [x] Tests (tracking seam): notices in, connection and errors out (including the repeated-error rule and the reset on a saved Event); the forwarder delivers notices in order and ends on the stop flag.
- [x] By-hand checks are listed under `## Comments`.

## Comments

Implemented by an agent; build, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean. What is left is seeing it against the game.

- New plain layer `src/tracking/` (documented in `docs/documents/architecture.md`): `service.rs` (`Service::start(data_dir, log_path)` opens `history.db`, calls `Tracker::start_live`, and one `tracker-bridge` thread forwards `Notice`s into an `async_channel`; `forward` is the testable loop; dropping the `Service` stops the thread and with it the `Tracker`) and `status.rs` (`Status::apply(&Notice)` gives the `Connection` and any new error).
- `main` keeps the `Service` outside `Application::run` and drops it after `run` returns, so a normal quit stops the tracker. If `history.db` cannot be opened the window still opens, with the reason in the error slot and "Not tracking". On a non-Windows build the service never starts (the tracker's live memory is Windows only).
- Decisions where the spec was silent: `Waiting` is shown as "Waiting for the game"; a startup error and a settings error are joined into one sticky message; `RunRebound` also clears the repeated-error memory (it is a successful save). The status bar's `Connection` enum moved from `ui/status_bar.rs` into `tracking/status.rs`.
- The app now has `tempfile` as a dev-dependency (for the failed-start test).

By hand, with `just run` (a debug build keeps its data in `.bazaar-tracker/`):

1. Start the app with the game closed: the status bar says "Waiting for the game" with a grey dot.
2. Start the game (or have it running first, then start the app): within a second or two it says "Tracking" with a green dot. Close the game: it goes back to "Waiting for the game".
3. Quit the app with Ctrl+Q while the game is attached: the window closes at once and `tracker.exe`/`bazaar-tracker-gpui.exe` does not linger in Task Manager.
4. Make `history.db` unopenable (for example set `BAZAAR_TRACKER_DATA_DIR` to a folder where `history.db` is a directory) and start the app: it opens, says "Not tracking" and shows the "Could not open history.db" error until dismissed with the cross.
5. Set `log_path` in `settings.toml` to a path that is a directory, restart: a "Could not read Player.log" error appears once, and does not come back after being dismissed.
6. Play: `logs/tracker.log` contains the "following ..." line with the path that was used.
