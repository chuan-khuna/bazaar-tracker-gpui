# 11: The game-memory trait on the real game

**What to build:** The game-memory trait is implemented on `unityspy-rs`, reading The Bazaar's state through the field paths in the notes. An example prints a live Snapshot (board, stash, skills, stats) and the current run's real id while the game is running, which a person checks against the game's screen. See the spec's Game memory decision and `docs/rea/bazaar-tracker/memory-reading.md` §2–4.

**Blocked by:** 08 (Reading values), 10 (Event and Snapshot types, and the game-memory trait)

**Status:** ready-for-human

- [ ] Every trait method is implemented with the field paths in `memory-reading.md` §2; items are sorted into board, stash and skills by inventory section and card type.
- [ ] All memory access is serialised behind one lock.
- [ ] While disconnected it tries to reconnect every 500 ms; it disconnects on process death or after five failed board reads in a row, and reports connection changes.
- [ ] Reads before the game has initialised a value return "not available yet", not an error that disconnects.
- [ ] An example prints a live Snapshot and run id; checked by hand in at least a shop, a combat and a PvP fight, with results recorded under `## Comments` in this file.
- [ ] Whether admin rights are needed (feasibility record, open question 2) is recorded under `## Comments`.
