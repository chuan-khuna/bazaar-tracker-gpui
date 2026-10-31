# 12: Run state machine: core

**What to build:** Given log lines and game memory, `bazaar-tracker` produces the Events of a Run: the Run starts and ends with the game's run, options offered and items or skills selected and sold become Events, and each Event is built when it is sent so its Snapshot reflects that moment. Tested entirely with recorded logs, the fake game memory and a fake clock. See the spec's Run state machine decision and `docs/rea/bazaar-tracker/log-and-events.md` §3.

**Blocked by:** 09 (Following and parsing `Player.log`), 10 (Event and Snapshot types, and the game-memory trait)

**Status:** resolved

- [x] A Run starts on run init, using the real run id when memory gives one and a `local-` id otherwise; it ends on the end-of-run states or an abandon.
- [x] Options presented settle 500 ms after entering an option state (or a new deal while in one); selections, skill selections and sales become Events linked to the options they came from.
- [x] Events are built when sent; their memory reads happen then.
- [x] Timers are cancelled by generation counters, so a newer line supersedes a pending one.
- [x] Time comes from an injectable clock.
- [x] Tests (run seam): recorded logs plus scripted memory produce the expected Events for a run start, several option rounds, a sale and a run end; a newer line cancels a pending settle.

## Comments

Implemented by an agent.

- `run::RunMachine` has no threads or timers of its own. It is given each log line (`on_line` / `on_raw_line`) and the passing of time (`tick`), keeps timers on an injectable `Clock` (`SystemClock`, or `ManualClock` in tests) and hands back `Output`s (`Event`, or `Rebound`). The tracker service (ticket 15) will drive `tick` and use `next_due_ms` to sleep.
- Events are built when they reach the front of an emit queue, which is when their memory reads happen. As in the original, a run start still waiting for the hero (up to six looks, 500 ms apart) holds back the events queued behind it.
- Generation counters: each `ScheduleSettle` bumps the settle generation and a timer only fires if its generation is still current; leaving the option states bumps it too.
- The rebinding of a `local-` run to its real id (`Output::Rebound`) is in `lifecycle.rs` because run identity belongs with starting a run; its tests and the combat parts are ticket 13. `combat.rs` is a placeholder until then.
- Ambiguity resolved: the notes do not say what the non-settle `FlushOptions` does. It reports the offered ids not yet reported, from a fresh board read, before a selection of one of them, and the Selected Event links to the options Event through `options_event_id`.
- Not built (out of the ticket): granted-item ids on Selected, and the original's per-line board read cost is kept only while the run id is unknown or local.
- `tests/run_replay.rs` plays the recorded `real-run-start.log` through the machine with scripted memory and a clock that follows the log's timestamps.
