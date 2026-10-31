# 01: Start the tracker and show the connection

**What to build:** The app depends on `bazaar-tracker`, starts its tracker service on a background thread when it opens, forwards the service's notices into gpui, and the status bar shows the connection state and the errors the service reports. See the spec's Starting the service, Bridge, Connection state and Errors decisions.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] The root package depends on `bazaar-tracker`; a new plain `src/tracking/` layer (no gpui) holds the bridge and the status logic.
- [ ] `main` opens `history.db` in the data folder, resolves `Player.log` from `log_path` and starts `Tracker::start_live`; the app still opens if `history.db` cannot be opened, with a sticky error and "Not tracking".
- [ ] One thread owns the `Tracker` and forwards its notices, in order, into an `async_channel`; dropping the service stops it and joins the tracker; `main` drops it when `run` returns.
- [ ] `TrackerApp` receives the notices without blocking the window and redraws on each.
- [ ] The status bar shows "Tracking" (success dot) when the game is attached, "Waiting for the game" (muted dot) when it is not, and "Not tracking" (muted dot) when the service is not running.
- [ ] `SaveFailed`, `LogError` and a failed start show as sticky errors; an error equal to the last one reported is not shown again until an Event is saved.
- [ ] Tests (tracking seam): notices in, connection and errors out (including the repeated-error rule and the reset on a saved Event); the forwarder delivers notices in order and ends on the stop flag.
- [ ] By-hand checks are listed under `## Comments`.

## Comments
