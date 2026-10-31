# 15: The headless tracker, end to end

**What to build:** A tracker service in `bazaar-tracker` runs the whole pipeline in the background: it follows `Player.log`, reads game memory, runs the run state machine and saves every Event to `history.db`, reporting connection changes and saved Events over a channel. `just track` (an example) runs it against the live game and prints each Event as it is saved. See the spec's Tracker service decision.

**Blocked by:** 11 (The game-memory trait on the real game), 13 (Run state machine: combat and rebinding), 14 (`history.db`)

**Status:** ready-for-agent

- [ ] The service starts and stops cleanly, owns its threads, and never blocks the caller.
- [ ] Every Event the run state machine sends is saved; a rebinding is applied to `history.db`.
- [ ] A failed save is retried and reported, not dropped silently.
- [ ] Connection changes (game found, game lost) and saved Events are sent over a channel.
- [ ] `just track` runs the service with the real game memory and the data folder's `history.db`, printing each saved Event.
- [ ] Tests (run seam): a recorded log plus scripted memory end as the expected rows in a temporary `history.db`.
- [ ] One real run is tracked by hand with `just track`; anything surprising is recorded under `## Comments` in this file.
