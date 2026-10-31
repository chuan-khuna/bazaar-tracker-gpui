# 05: Log panel

**What to build:** A toggleable bottom panel under the main island, like an editor's terminal panel, streaming the last 2,000 log rows live, with a fixed details block pinned at its top. Added after the spec was written, at the user's request; it is not in the spec's user stories.

**Blocked by:** 04 (done after 06, at the user's request)

**Status:** ready-for-human

- [x] A ring buffer keeps the last 2,000 rows; older rows drop off.
- [x] Rows come from the app's own `tracing` output (including the tracker crate's records), through a tracing layer that feeds the buffer as well as `logs/tracker.log`, and from the tracker service's notices (connection changes, saved Events, rebinds, save failures, log errors).
- [x] Each row reads as `HH:MM:SS`, a level or kind, and a plain sentence, such as `Connected to The Bazaar (pid 11776, Unity 6000.3.11)`, `Combat ended: won against Mak (PvP)`, `Bought Pasta Maker (Bronze, Medium) for 4 gold`. No Debug structs or JSON. Card names come from the catalog, falling back to the template id, then the instance id. Errors and warnings are drawn in the theme's error and warning-like roles.
- [x] A pinned details block does not scroll with the log: process name and pid, Unity version and offset table (and whether verified), Mono module and root-domain addresses (adding the smallest accessor to the tracker if it does not expose them), attach state, the `Player.log` path followed, the `history.db` path, and the time of the last Snapshot or Event.
- [x] The panel is toggled from the menu, the palette and a shortcut, following the commands pattern; whether it is open and its height are saved in the settings `[window]` table.
- [x] The log follows the newest row unless the player has scrolled up, and resumes when they scroll back to the bottom.
- [x] Tests (no window): the 2,000-row cap, the sentence for each notice and Event kind, the details block's updates, the `commands` list and the settings keys.
- [x] By-hand checks are listed under `## Comments`.

## Comments

Implemented by an agent, **after ticket 06** (06 was done first because the user was looking at the main island; this work had been started and was set aside with `git stash`, then resumed). Build, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean. The panel has never been seen on screen.

- **Rows** (`tracking/log.rs`, `tracking/narrate.rs`, `tracking/feed.rs`): `LogBuffer` keeps the last 2,000 `LogRow`s (time, kind, sentence). Two sources: a `tracing` layer (`feed::channel`, added in `shell/logging.rs` beside the file layer, info level and above, so it carries the tracker crate's records too; `logging::init` now takes the layer and installs the level filter itself, and still reports a log file that cannot be opened while keeping the layer), and the tracker's notices through a `Narrator`, which turns each into a plain sentence (connection, each Event kind, rebinds, save failures, log errors; `LiveSnapshot` is not logged). The narrator remembers just enough to read well: whether a selection came from a shop (a purchase is `Bought X (Tier, Size) for N gold`, otherwise `Took X`), what an item sold was (size and sell price from the previous Snapshot), and who a combat was against. Names come from the card catalog, falling back to the template id then the instance id. Errors are drawn in the error colour, warnings in the `burn` colour (the theme has no warning role), event kind labels in the accent colour.
- **Pinned details** (`tracking/details.rs`, drawn above the rows and not scrolling with them): state, process name and pid, Unity version, offset table and whether it is verified, Mono module and its base, root domain, the `Player.log` followed, the `history.db`, the time of the last saved Event (every Event, not only ones with a Snapshot). To give it the facts, the tracker needed its smallest accessor: `ConnectionChange::Connected` now carries `details: AttachDetails` (process name, Unity version from the executable, offset table and `verified`, Mono module name and base, root domain), filled in where the game memory attaches. `unity_version` in the change is still the offset table name, as before. `Service` also reports the two paths in its `Link`.
- **Layout and commands**: a bottom panel under the main island only (not under the sidebar or detail), with a drag handle between them. **Show log** is a command in the menu and palette (group with the other toggles, on-dot when open) and `Ctrl+J` (`secondary-j`). `[window]` has two new keys, `log_open` (default false) and `log_height` (default 220, clamped to 120..600, also on load), saved as they change; **Reset layout** restores them. The log follows the newest row unless the player has scrolled up, and follows again when scrolled back to the bottom (checked on each row against the scroll handle's offset; the check is not covered by a test as it needs a window).
- Fixes found by the tests: a plural ("losss") in the run-ended sentence.

By hand, with `just run` (and the game for the game-specific rows):

1. `Ctrl+J`, the menu's **Show log** and the palette's **Show log** each open and close the panel under the main island; the menu row shows its shortcut and an on-dot. Close and reopen the app: it comes back open or closed as you left it, at the height you set. **Reset layout** closes it.
2. Drag the strip between the main island and the panel: the height follows the pointer and stops at about 120 and 600 px.
3. With the app just started, the log has the start-up lines (`Bazaar Tracker 0.1.0 started`, `data folder: ...`, `following ...`) as readable sentences with an `HH:MM:SS` time. Check the same lines are in `logs/tracker.log`.
4. Start the game: a row `Connected to The Bazaar (pid N, Unity 6000.x)` appears and the pinned block fills in (process, Unity, offsets with `not verified`, Mono module and root domain addresses). Compare the addresses and versions with the `just spike` output in record .003.
5. Play: rows such as `Run started: Jules, Unranked`, `Offered 3 options (Choice): ...`, `Bought X (Bronze, Medium) for 4 gold`, `Sold ...`, `Combat started: PvP against Mak`, `Combat ended: won against Mak (PvP)`, `Run ended: victory (10 wins, 2 losses)`. Check the card names are names (not ids), that a free choice reads `Took ...` and that no row looks like a struct or JSON.
6. Close the game: a warning row `Lost the game: ...` in the warning colour. Cause an error (point `log_path` at a folder): an error row in the error colour.
7. Scroll up in the log while the game runs: it stops following. Scroll back to the bottom: it follows again.
8. Let it run a long time or replay a long log: the panel stays responsive; the oldest rows are the ones that drop off.
