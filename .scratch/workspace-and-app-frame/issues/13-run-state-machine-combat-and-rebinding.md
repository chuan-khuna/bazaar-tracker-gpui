# 13: Run state machine: combat and rebinding

**What to build:** The run state machine records combats and fixes up run identity. Entering a combat captures both boards as Snapshots, combat start and end become Events with the outcome, a ranked run end waits for the new rating, and a Run that started under a `local-` id is rebound to its real id when memory provides it. See the spec's Run state machine decision and `docs/rea/bazaar-tracker/log-and-events.md` §3.3–3.6.

**Blocked by:** 12 (Run state machine: core)

**Status:** resolved

- [x] Boards are captured 350 ms after entering combat (or on new spawns before playback), cancelled by a generation counter.
- [x] Combat start carries the player's and opponent's Snapshots, PvP opponent name, hero and rating when it is a PvP fight.
- [x] Combat end carries the outcome; when neither the simulation nor HP decides it, it retries after 1200 ms.
- [x] A ranked run end waits up to 4000 ms for the rating to change before the run-ended Event.
- [x] A Run with a `local-` id is rebound to the real id once read, and the rebinding is reported so history can follow.
- [x] Tests (run seam): a PvE and a PvP combat with Snapshots; an undecided outcome resolved by the retry; a ranked end with a rating change; a run whose real id arrives late.

## Comments

Implemented by an agent.

- `run/combat.rs`: boards are captured 350 ms after entering a combat state, or 350 ms after cards spawn before the fight is played; "Combat prepared" starts the fight (cancelling a waiting capture by its generation counter) and sends CombatStart with the captured player and opponent Snapshots; a PvP fight (a `PVPCombatState`, or a `pvp_` card placed) also carries the opponent's name, hero, rating and level. Leaving the combat state sends CombatEnd, which refers to its CombatStart.
- The outcome is decided from the simulation (ignored with `sim_diag: stale (frame mismatch)` when its frame count is not the expected one), then the win/loss counters, the run state, then HP. If none decides it, the end is tried again after 1200 ms and sent as it then stands (possibly Unknown). In PvP the counters are at least one past their pre-fight value for the winning side.
- A ranked run end looks at the rating every 500 ms and waits up to 4000 ms for it to change from the rating at run start, then sends RunEnded. This is a small refinement of the original's single 4000 ms wait: it sends as soon as the rating changes.
- Rebinding (`Output::Rebound`) was written with the run identity code in ticket 12 and is tested here: a run under a `local-` id is rebound when memory gives its real id, a real or ended run is not.
- The combat-frames Event (the original's `ManagedJson` tree) is out of scope, so a combat produces CombatStart and CombatEnd only.
- `tests/run_replay.rs` now also plays the recorded PvE and PvP combats.
