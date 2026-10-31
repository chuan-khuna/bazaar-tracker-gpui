# 09: Following and parsing `Player.log`

**What to build:** `bazaar-tracker` follows The Bazaar's `Player.log` as the game writes it and turns each relevant line into a typed log line, surviving game restarts that truncate or replace the file. See the spec's Log following decision and `docs/rea/bazaar-tracker/log-and-events.md` §1–2.

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** resolved

- [x] The follower starts at the end of the file, waits for a missing file, and polls every 500 ms.
- [x] It resets to the start when the file shrinks or its tail signature no longer matches.
- [x] It reads at most 16 MiB per pass and holds back a partial last line until it is complete.
- [x] The timestamp prefix is stripped and lines are parsed with the six line patterns from the notes, each guarded by a substring check, into typed log lines (state changed, skill selected, game-sim summary, card placed, command completed, combat prepared).
- [x] The default log path is the game's `Player.log` under `LocalLow\Tempo Storm\The Bazaar`; a `log_path` setting overrides it.
- [x] Tests (log seam): matching and near-miss fixture lines for every pattern; a temporary file that grows, ends in a partial line, is truncated and is replaced.
- [x] A short real `Player.log` excerpt is added to the recorded-logs fixtures.

## Comments

Implemented by an agent.

- `LogLine` also has `RunInitialized` and `CombatCompleted` (the notes' two checks without a regex), since the run state machine starts a Run on run init; the spec's "six patterns" are the six with regexes.
- One deliberate refinement of "starts at the end of the file": a file that was missing when the follower looked, and appears later, is read from its first line, because everything in it is new. A file that already exists at start is followed from its end.
- Fixtures: `tests/fixtures/logs/real-run-start.log` (a PvE run start through the first combat) and `real-pvp-combat.log` are lines 158-300 and 420-470 of a real `Player-prev.log`. They contain no account details. The sample is from a Unity 6000.3.11f1 game, which is the Unity line the Mono offsets (ticket 06) will meet.
- The end-of-run state names (`EndRunVictoryState`, `EndRunDefeatState`) and `AbandonRunCommand` are still not in any real sample.
