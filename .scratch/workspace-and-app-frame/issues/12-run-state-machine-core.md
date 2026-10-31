# 12: Run state machine: core

**What to build:** Given log lines and game memory, `bazaar-tracker` produces the Events of a Run: the Run starts and ends with the game's run, options offered and items or skills selected and sold become Events, and each Event is built when it is sent so its Snapshot reflects that moment. Tested entirely with recorded logs, the fake game memory and a fake clock. See the spec's Run state machine decision and `docs/rea/bazaar-tracker/log-and-events.md` §3.

**Blocked by:** 09 (Following and parsing `Player.log`), 10 (Event and Snapshot types, and the game-memory trait)

**Status:** ready-for-agent

- [ ] A Run starts on run init, using the real run id when memory gives one and a `local-` id otherwise; it ends on the end-of-run states or an abandon.
- [ ] Options presented settle 500 ms after entering an option state (or a new deal while in one); selections, skill selections and sales become Events linked to the options they came from.
- [ ] Events are built when sent; their memory reads happen then.
- [ ] Timers are cancelled by generation counters, so a newer line supersedes a pending one.
- [ ] Time comes from an injectable clock.
- [ ] Tests (run seam): recorded logs plus scripted memory produce the expected Events for a run start, several option rounds, a sale and a run end; a newer line cancels a pending settle.
