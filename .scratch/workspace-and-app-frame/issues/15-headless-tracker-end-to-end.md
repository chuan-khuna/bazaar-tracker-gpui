# 15: The headless tracker, end to end

**What to build:** A tracker service in `bazaar-tracker` runs the whole pipeline in the background: it follows `Player.log`, reads game memory, runs the run state machine and saves every Event to `history.db`, reporting connection changes and saved Events over a channel. `just track` (an example) runs it against the live game and prints each Event as it is saved. See the spec's Tracker service decision.

**Blocked by:** 11 (The game-memory trait on the real game), 13 (Run state machine: combat and rebinding), 14 (`history.db`)

**Status:** ready-for-human

- [x] The service starts and stops cleanly, owns its threads, and never blocks the caller.
- [x] Every Event the run state machine sends is saved; a rebinding is applied to `history.db`.
- [x] A failed save is retried and reported, not dropped silently.
- [x] Connection changes (game found, game lost) and saved Events are sent over a channel.
- [x] `just track` runs the service with the real game memory and the data folder's `history.db`, printing each saved Event.
- [x] Tests (run seam): a recorded log plus scripted memory end as the expected rows in a temporary `history.db`.
- [ ] One real run is tracked by hand with `just track`; anything surprising is recorded under `## Comments` in this file.

## Comments

Implemented by an agent; everything except the by-hand run is done and tested against a fake game and a temporary `history.db`.

- `bazaar_tracker::tracker::Tracker`: `Tracker::start(config, memory, store, clock, connection)` (generic, used by tests) and `Tracker::start_live(config, Arc<History>)` (Windows; builds `BazaarMemory`, runs its 500 ms connection loop and passes its attach/detach changes on). Threads: the log follower, the run state machine with the store, a connection forwarder and, live, the game connection loop. `start` returns at once, `stop()` (or dropping) joins all threads promptly (the forwarders poll a stop flag), and nothing blocks the caller: it only reads `Tracker::notices()`.
- `Notice`: `Connection(Connected | Disconnected)`, `EventSaved(Event)`, `RunRebound`, `SaveFailed { what, error, retry_in }` and `LogError`. Losing the game resets the run state machine (as the original's `GameTracker.Reset()`); attaching does not.
- Saves and rebinds go through the `Store` trait (`Arc<History>` implements it) in the order the machine produced them. A failure is reported as `SaveFailed`, stops the queue at that item, and is retried after `retry_delay` doubling up to `max_retry_delay` (1 s and 30 s by default); later items wait behind it, so nothing is dropped or reordered. Saving an Event twice is harmless (ticket 14).
- `just track [-- --log <path>]` runs `examples/track.rs`: `start_live` against the data folder's `history.db` (resolved like the app does) printing each notice as it comes.
- Tests: a recorded log plus scripted memory end as the same rows, in the same order, in a temporary `history.db`; a flaky store; connection changes and the reset; a rebinding applied to the history; clean start and stop. The timing in them is real (tens of milliseconds), and they passed repeatedly.

To do by hand: run `just track` while playing one real run and record anything surprising here (the live game memory, ticket 11, is the part that is not yet checked against the real game).
