# 11: The game-memory trait on the real game

**What to build:** The game-memory trait is implemented on `unityspy-rs`, reading The Bazaar's state through the field paths in the notes. An example prints a live Snapshot (board, stash, skills, stats) and the current run's real id while the game is running, which a person checks against the game's screen. See the spec's Game memory decision and `docs/rea/bazaar-tracker/memory-reading.md` §2–4.

**Blocked by:** 08 (Reading values), 10 (Event and Snapshot types, and the game-memory trait)

**Status:** ready-for-human

- [x] Every trait method is implemented with the field paths in `memory-reading.md` §2; items are sorted into board, stash and skills by inventory section and card type.
- [x] All memory access is serialised behind one lock.
- [x] While disconnected it tries to reconnect every 500 ms; it disconnects on process death or after five failed board reads in a row, and reports connection changes.
- [x] Reads before the game has initialised a value return "not available yet", not an error that disconnects.
- [ ] An example prints a live Snapshot and run id; checked by hand in at least a shop, a combat and a PvP fight, with results recorded under `## Comments` in this file.
- [ ] Whether admin rights are needed (feasibility record, open question 2) is recorded under `## Comments`.

## Comments

Implemented by an agent; needs a human with the game. Nothing here could be run against the real game (it was not running), so every field path below is unverified.

Done and tested without the game:
- `memory::game::BazaarMemory` implements `GameMemory` for all seven reads with the field paths of `memory-reading.md` §2. Items are sorted into board, stash and skills by owner, inventory section and card type, with the notes' rules for the opponent (kept encounter cards only for the current encounter id, deduplicated by template). All access is behind one lock; clones of the handle share it.
- `tick()` attaches when detached and detaches when the process has exited; `run_connection_loop` calls it every 500 ms until stopped; `report_changes_to` sends `ConnectionChange::Connected/Disconnected`. Five board reads in a row that fail detach (`BoardFailures`, tested). A value the game has not set (no run, class not initialised, null reference) is `Ok(None)` and neither counts as a failure nor detaches.
- Dictionaries and hash sets of ints are read through their entry classes, falling back to the original's raw layout (hash 0, key 8, value 12; hash 0, value 8) when the entry class's generic types are not resolved. `Guid`s and `Nullable<enum>` are read raw as the original does. The rank record uses the original's offset scan and plausibility ranges (tested).
- `cargo run -p bazaar-tracker --example snapshot [-- --watch]` (also `just snapshot`) prints the run id, selected hero, board state with both sides' board, stash and skills, stats, PvP opponent, rank and last combat.

To do by hand (the unticked boxes):
1. Start The Bazaar, then run `just snapshot -- --watch` and compare the output with the screen in a shop, in a PvE combat and in a PvP fight. Record what matches and what does not here. Likely trouble spots: `Data.Entities` is read with the typed dictionary reader only (no raw fallback, because its key type is unknown); the `StateName` field name; `Observable<T>._value` for the rank and `RunConfig`.
2. Admin rights (feasibility record, open question 2): the reader opens the game with `PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION` only and never elevates. Run it from a normal (non-admin) terminal and record whether attaching works. From a local `Player.log`, the installed game is Unity 6000.3.11f1, a Unity 6 build, which uses the unverified Unity 6 offsets table (ticket 06).
