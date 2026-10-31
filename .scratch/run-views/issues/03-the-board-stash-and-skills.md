# 03: The main island: run header, board, stash and skills

**What to build:** The main island shows the selected Run's header and its board, stash and skills at the selected moment (for now the latest one: live, or the end of a finished Run). See the spec's The moment, Board layout, Badges, Names and Header decisions and the folder-structure record's UI section.

**Blocked by:** 02

**Status:** ready-for-human

- [x] Selecting a Run loads its Events from `history.db`; saved Events for the selected Run are appended as they arrive.
- [x] `moment` gives the Snapshot shown: the latest Event's that has one; an empty board for a Run with none.
- [x] The header shows start time, hero, rank and rating, the rating change when ended, the LIVE chip, the outcome, `Day N`, the record and the per-day timeline.
- [x] Items sit on a 10-slot grid by socket (Small 1, Medium 2, Large 3); collisions and items with no socket go to the first gap; what fits nowhere is counted; `SocketEffect` cards are not drawn.
- [x] Tiles have a tier border, the name (with the enchantment), badges (damage, shield, heal, burn, poison, regen, ammo, `x2`), the sell price bottom-left and the cooldown bottom-right, with no art; hovering shows a tooltip of everything else.
- [x] The stash is drawn under the board and folds with its heading, saved in `stash_open`; skills are a row of tier-bordered chips under it.
- [x] The board header says which moment this is and shows its health, gold and level.
- [x] Names come from the card catalog, loaded once off the UI thread; until it loads, or for an unknown card, the template id (then instance id) is shown, and the view redraws when it arrives.
- [x] Tests (board seam): moment lookup, slot layout, badges and tooltip lines, name fallbacks, header values and timeline from Events.
- [x] By-hand checks are listed under `## Comments`.

## Comments

Implemented by an agent; build, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean. The drawing (`ui/board.rs`, `ui/run_header.rs`) has never been seen on screen, so everything about how it looks is a by-hand check.

- View models in `src/tracking/`: `session.rs` (now also holds the selected Run's Events; `select` takes the history and loads them, saved Events of the selected Run are appended, a rebinding reloads), `moment.rs` (`at(events, selected)`: latest Snapshot, or nearest at or before the selected Event; the selected-Event half is tested here and wired to clicks in ticket 04), `board.rs` (slot layout, badges, tooltips, the `BoardView`), `header.rs`, `names.rs`, `when.rs` (`long` start time).
- Decisions where the records were silent: a rank is shown as `Gold 3 · 812` (division as the number the game gives); a stash item is shown in order whatever its socket; sockets are placed first and displaced items take the first gap; the badge for ammo shows the current `Ammo`; the tooltip lists every non-zero attribute that the tile does not already show, under its raw name (so the unnamed `100` appears as `100`); skills are text chips with a tier border; the board header reads `live`, `end of run` or `Event #n`, plus `Day d Hour h` of the Snapshot.
- The catalog is loaded with `CatalogHandle::start` from `<USERPROFILE>\AppData\LocalLow\Tempo Storm\The Bazaar`; the window waits for it on the background executor for up to two minutes and redraws when it arrives.
- A tooltip is a small themed view (`widgets::hint`) rather than gpui-component's, which reads that kit's own theme.
- Unrelated fix made along the way: the catalog test `game_data_db_is_read_from_a_copy_and_the_copy_is_deleted` failed on about half of the runs, because Windows sometimes refuses to delete the temporary copy at once. The copy is now deleted with a few retries (`TempCopy::drop` in `crates/bazaar-tracker/src/catalog/mod.rs`) and the test waits a moment for it.
- While this was built the user's own copy of the app was running from `target\debug`, so these builds went to a separate target folder.

By hand, with `just run` and the game (or existing Runs in `.bazaar-tracker/history.db`):

1. Click a Run: the main island shows the header (start time, hero, rank and rating if the Run has them, LIVE chip on a live Run, outcome, `Day N`, record) and, under it, BOARD, STASH and SKILLS.
2. The header's day timeline has a dot per day, green or red where that day had a finished combat. Check it against a Run you know.
3. The board is ten slots across. A Medium item should be two slots wide and a Large three, in the position the game puts them. Compare with the game window (the shop screenshot in record 2026-10-31.002 is the reference: 4 items, 2 Small and 2 Medium).
4. Each tile has a border in its tier colour, the name (enchantment above it), badges on top (damage, shield, heal, burn, poison, regen, ammo and `x2`), sell price bottom-left and cooldown bottom-right (e.g. `3s`). Hover a tile: the tooltip lists tier, size, type, enchantment, tags and the other non-zero attributes (including a bare `100`).
5. Names: after a moment the tiles show card names rather than ids like `itm_...` or GUIDs. If they never do, check the `card catalog` lines in `logs/tracker.log`.
6. Click `STASH`: it folds and unfolds, and stays so after a restart.
7. Play a live Run: the board and stats (health, gold, level) update as Events arrive (on each Options/Selected/Sold), the header shows LIVE. Between Events the board is not polled, so it can lag the game a little (by design).
8. A finished Run shows `end of run` and its last board. The SocketEffect card (the Diamond `eft_` item seen in record .002) should not be drawn.
9. Resize the window narrow: the header wraps rather than overflows.
