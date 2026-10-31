# 05: Log panel

**What to build:** A toggleable bottom panel under the main island, like an editor's terminal panel, streaming the last 2,000 log rows live, with a fixed details block pinned at its top. Added after the spec was written, at the user's request; it is not in the spec's user stories.

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] A ring buffer keeps the last 2,000 rows; older rows drop off.
- [ ] Rows come from the app's own `tracing` output (including the tracker crate's records), through a tracing layer that feeds the buffer as well as `logs/tracker.log`, and from the tracker service's notices (connection changes, saved Events, rebinds, save failures, log errors).
- [ ] Each row reads as `HH:MM:SS`, a level or kind, and a plain sentence, such as `Connected to The Bazaar (pid 11776, Unity 6000.3.11)`, `Combat ended: won against Mak (PvP)`, `Bought Pasta Maker (Bronze, Medium) for 4 gold`. No Debug structs or JSON. Card names come from the catalog, falling back to the template id, then the instance id. Errors and warnings are drawn in the theme's error and warning-like roles.
- [ ] A pinned details block does not scroll with the log: process name and pid, Unity version and offset table (and whether verified), Mono module and root-domain addresses (adding the smallest accessor to the tracker if it does not expose them), attach state, the `Player.log` path followed, the `history.db` path, and the time of the last Snapshot or Event.
- [ ] The panel is toggled from the menu, the palette and a shortcut, following the commands pattern; whether it is open and its height are saved in the settings `[window]` table.
- [ ] The log follows the newest row unless the player has scrolled up, and resumes when they scroll back to the bottom.
- [ ] Tests (no window): the 2,000-row cap, the sentence for each notice and Event kind, the details block's updates, the `commands` list and the settings keys.
- [ ] By-hand checks are listed under `## Comments`.

## Comments
