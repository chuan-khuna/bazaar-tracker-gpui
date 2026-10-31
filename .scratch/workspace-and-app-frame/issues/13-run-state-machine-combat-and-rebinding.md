# 13: Run state machine: combat and rebinding

**What to build:** The run state machine records combats and fixes up run identity. Entering a combat captures both boards as Snapshots, combat start and end become Events with the outcome, a ranked run end waits for the new rating, and a Run that started under a `local-` id is rebound to its real id when memory provides it. See the spec's Run state machine decision and `docs/rea/bazaar-tracker/log-and-events.md` §3.3–3.6.

**Blocked by:** 12 (Run state machine: core)

**Status:** ready-for-agent

- [ ] Boards are captured 350 ms after entering combat (or on new spawns before playback), cancelled by a generation counter.
- [ ] Combat start carries the player's and opponent's Snapshots, PvP opponent name, hero and rating when it is a PvP fight.
- [ ] Combat end carries the outcome; when neither the simulation nor HP decides it, it retries after 1200 ms.
- [ ] A ranked run end waits up to 4000 ms for the rating to change before the run-ended Event.
- [ ] A Run with a `local-` id is rebound to the real id once read, and the rebinding is reported so history can follow.
- [ ] Tests (run seam): a PvE and a PvP combat with Snapshots; an undecided outcome resolved by the retry; a ranked end with a rating change; a run whose real id arrives late.
