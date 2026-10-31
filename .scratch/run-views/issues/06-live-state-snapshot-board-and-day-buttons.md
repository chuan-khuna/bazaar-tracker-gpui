# 06: Live game state on top, the snapshot board below, day buttons

**What to build:** The main island is split in two. The top section, "Live", shows the game's state as it is now (the shop's offers, or the encounter options, or the opponent's board in a combat, with the player's own items), polled about once a second while the tracker is attached and a run is live. Under it, the existing board view moves down as "Snapshot". The run header's day status becomes a row of day buttons, one per day of the run; clicking one shows that day's last snapshot in the Snapshot section. Added after the spec was written, at the user's request, from an annotated screenshot (`I:\Screenshot 2026-10-31 112647.png`). It reverses the spec's "the live board is not polled" decision for the Live section only; the Snapshot section is unchanged.

**Blocked by:** 04

**Status:** ready-for-human

- [x] The tracker service polls a board read about every second while the game is attached and reports it as a new notice (`Notice::LiveSnapshot`) only when it has changed; nothing is saved to `history.db`; the run state machine's own reads are made in the same thread, so they are not disturbed.
- [x] The Live section shows, from that notice: in a shop (`Encounter` with items on offer) the shop's items as tiles in their own row; at a `Choice` the encounter options by name; in a combat the opponent's board if it was read; in every case the player's own board, stash and skills as they are now.
- [x] Without the game attached, or with no live run, the Live section shows a short placeholder.
- [x] The Snapshot section is the current board view with its own label, under the Live section; it behaves as before.
- [x] The day status in the run header is a row of day buttons (one per day, with the day's combat result as a dot); clicking one highlights it and shows that day's last snapshot (or the nearest before it if the day has none); clicking it again goes back to the latest.
- [x] Choosing an Event moves the selected day to that Event's day; choosing a day clears the chosen Event.
- [x] Tests (no window): the live view model for shop, choice, combat and other states; the day-to-snapshot lookup; the day and Event selection interplay; the tracker's live notice with the fake memory (sent on change only, stopped on disconnect).
- [x] By-hand checks are listed under `## Comments`.

## Comments

Implemented by an agent, **before ticket 05** (the user was looking at the main island; 05's unfinished work was set aside with `git stash` and is resumed afterwards). Build, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean. The drawing has never been seen on screen.

- **Tracker.** `Notice::LiveSnapshot(Option<BoardState>)`, sent by the service's machine thread about once a second (`TrackerConfig::live_interval`, default 1 s, `None` turns it off) only while the game is attached and only when the read differs from the last one sent (`None` when the game has no board). The read is `RunMachine::read_live`, made on the machine's own thread between its own work, so it cannot interleave with the machine's reads; the polling time comes from the injected clock. Nothing is saved. One thing to know: a failing poll counts towards the memory's existing "five board reads in a row failed, so detach" rule, as any failing board read does; failed polls are otherwise ignored.
- **Live view model** (`tracking/live.rs`): the state words come from the game's run state (`Encounter` is "Shop", `Choice` is "Encounter choice", `Combat`/`PVPCombat` is "Combat", others as named); offers are the `selection_set` ids resolved through the read's entities: `itm_` ones as tiles in their own row (laid out in order), anything else as cards by name (card type and tier as the detail), an id with no entity by its id; the opponent's board only in a combat state (the game keeps the last opponent's values in a shop, record .002); our own board, stash and skills through the same view as the Snapshot section. The section shows only with the game attached and a live Run (`Session::live`); otherwise a one-line placeholder.
- **Layout.** The main island scrolls as a whole: Live (LIVE label, state and time, stats, offers, opponent, our items), a rule, then Snapshot (header with day buttons, SNAPSHOT label, board, stash, skills, and the Events list at a fixed 220 px with its own scroll). The stash fold is shared by both sections. Window 1000x640 does not fit both at once without scrolling.
- **Day buttons** replace "Day N" and the dots: one `Day N` button per day with the day's combat result as a dot; the chosen one is outlined and highlighted. A day shows its last snapshot (the last Event on or before it that has one, so a day with no snapshot shows the nearest before it). Choosing a day clears the chosen Event; choosing an Event moves the day to its day, and letting go of the Event lets go of the day; clicking the chosen day again goes back to the latest. The board header reads `end of day N · Hour H`. Keyboard and palette access were not added.
- The Run header lost its separate "Day N" text and its bottom border; its record stays.

By hand, with `just run` and the game in a Run:

1. In a shop: the LIVE section's SHOP row shows the shop's items as tiles; compare names, tiers and sizes with the game. Your own board, stash and skills are shown under it and match the game.
2. At an encounter choice: the CHOICES list matches the options on screen by name (if names show as `enc_...` ids, the catalog has not loaded or does not know them: note which).
3. In a combat: the state reads Combat and, if the read has it, an OPPONENT row shows their board. Outside a combat no opponent row shows.
4. Make a purchase or move an item: the Live section changes within about a second (the status in `[LIVE]` and the stats too); the Snapshot section below does not change until an Event is saved.
5. Close the game: the Live section turns to the placeholder. Start the app with the game closed: the placeholder says it is waiting for the game. Between runs (menu): "No run is being played right now."
6. Day buttons: one per day of the Run, in order; click Day 2: the button highlights and the Snapshot board shows the end of day 2 (header says `end of day 2`); click it again: back to the latest. Click an Event of day 3: Day 3 highlights. Click a day while an Event is chosen: the Event highlight goes.
7. Resize the window small: the main island scrolls rather than clipping.
8. Check the game does not stutter or detach while the app polls (watch the status bar stays "Tracking" for a whole run).

**2026-10-31, first look on screen:** with the game in a run, LIVE said "No run is being played right now", the Snapshot board was empty and the run showed "Unknown hero" (screenshot `I:\Screenshot 2026-10-31 121530.png`), while `just snapshot` read everything. Cause: the app attached while the game was still loading, so `BoardManager` and friends were found and cached before the runtime gave them a vtable; `unityspy-rs` caches classes, so every static read of them stayed `NotInitialised`, which the memory treats as "nothing yet" — empty Snapshots and `LiveSnapshot(None)` for the whole session. Fix: `Mono::static_data` looks the vtable up again in the live class when the cached one has none (test `a_class_cached_before_it_was_initialised_is_read_once_it_is`). Events saved during the broken session keep their empty Snapshots.
