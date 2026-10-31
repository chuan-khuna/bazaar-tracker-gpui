# 09: Following and parsing `Player.log`

**What to build:** `bazaar-tracker` follows The Bazaar's `Player.log` as the game writes it and turns each relevant line into a typed log line, surviving game restarts that truncate or replace the file. See the spec's Log following decision and `docs/rea/bazaar-tracker/log-and-events.md` §1–2.

**Blocked by:** 01 (Workspace scaffold and first window)

**Status:** ready-for-agent

- [ ] The follower starts at the end of the file, waits for a missing file, and polls every 500 ms.
- [ ] It resets to the start when the file shrinks or its tail signature no longer matches.
- [ ] It reads at most 16 MiB per pass and holds back a partial last line until it is complete.
- [ ] The timestamp prefix is stripped and lines are parsed with the six line patterns from the notes, each guarded by a substring check, into typed log lines (state changed, skill selected, game-sim summary, card placed, command completed, combat prepared).
- [ ] The default log path is the game's `Player.log` under `LocalLow\Tempo Storm\The Bazaar`; a `log_path` setting overrides it.
- [ ] Tests (log seam): matching and near-miss fixture lines for every pattern; a temporary file that grows, ends in a partial line, is truncated and is replaced.
- [ ] A short real `Player.log` excerpt is added to the recorded-logs fixtures.
