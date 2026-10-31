# 04: The events list, and event selection driving the board and detail

**What to build:** The main island lists the selected Run's Events; selecting one moves the board, stash and skills to the nearest Snapshot at or before it and fills the detail island with what is specific to the Event. See the spec's The moment, Event rows, Detail and Scrolling decisions.

**Blocked by:** 03

**Status:** ready-for-human

- [x] One row per Event: `m:ss` into the Run, `D<day> H<hour>` (day only when it changes), the kind and a short description; Selected and Sold rows are sub-rows; combat results use the win or loss colour.
- [x] Clicking an Event selects it (highlighted); clicking it again clears the selection. Selection is per Run and is cleared when another Run is selected.
- [x] With an Event selected, `moment` gives the nearest Snapshot at or before it (CombatStart contributes its player side), and the board header names that Event.
- [x] The list scrolls, and stays at the bottom as Events arrive while no Event is selected and the Run is live.
- [x] The detail island shows: for Options, the options as cards with the pick marked and "Picked 1 of N"; for Selected, its options Event's cards; for CombatStart, the opponent's facts and both boards; for CombatEnd, the result and the opponent board of its CombatStart; for Run started, Run ended and Rerolled, their facts. It says so if the Event is no longer in the Run.
- [x] The empty states remain when no Event is selected.
- [x] Tests (events seam): row text for every kind, sub-rows, day shown on change, `m:ss`, nearest-Snapshot lookup from an Event with none of its own, detail for each kind including a missing options or start Event, selection clearing.
- [x] By-hand checks are listed under `## Comments`.

## Comments

Implemented by an agent; build, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean. The drawing (`ui/events.rs`, `ui/detail.rs`, `ui/board.rs`) has never been seen on screen.

- View models: `tracking/events.rs` (`rows`), `tracking/detail.rs` (`detail`), and `Session::toggle_event` / `selected_event` in `tracking/session.rs`. `moment::at` (ticket 03) now gets the selected Event through `board::view`.
- Selection rules: choosing an Event of the Run selects it; choosing the same one again lets go; choosing another Run, or a new Run going live, clears it; reselecting the same Run, or a rebinding, keeps it. Only an Event that is in the Run can be chosen.
- Where the records were silent: the hour column shows `D<day> H<hour>` when the day changes, `H<hour>` when only the hour does, and nothing otherwise (the mockup also showed hour separators and encounter banners, which were left out); a Selected row reads `Name (Tier): Item`; an Options Event with no pick says `N offered, none picked`; a Rerolled Event says the offers were refreshed; a combat's boards show only the items (SocketEffect cards are left out, as on the board).
- The Events list follows a live Run to the bottom by calling `ScrollHandle::scroll_to_bottom` on each notice while no Event is chosen. It does not distinguish "the player scrolled up" (the ticket's text does not ask for that; ticket 05's log panel does).
- The detail island's "Event is gone" message is reachable only if the Events are reloaded without it; it is covered by a test.

By hand, with `just run` and a Run that has Events (an old one from `history.db` is enough):

1. Select a Run: its Events are listed under the board, with time into the Run, day/hour, kind and description. Selected and Sold rows are indented with an arrow. Combat results are green (victory) or red (defeat).
2. Click an Event: the row highlights, the board header says `Event #n`, and the board, stash and skills change to that moment. Click a Combat End or Run Ended row: the board shows the last Snapshot before it (not blank). Click the first row (Run Started): the board is empty.
3. The detail island, with an Options row selected, lists the cards offered, the picked one marked with a tick and a lighter background, and the banner `Picked 1 of N`. With a Selected row, the same cards. Compare with what you chose in the game.
4. With a Combat Start row: the opponent facts (mode, name, hero, rating, level) and the OPPONENT and PLAYER boards. With the Combat End: result, health after, record, and the opponent's board. Check the opponent board against the game.
5. Click the selected row again: the highlight goes, the board returns to the latest moment, and the detail island shows its empty state.
6. Watch a live Run: new rows appear at the bottom and the list stays scrolled to the newest. Click an Event partway: the list stops following and the board stays on that Event while new ones arrive; click it again to go back to live.
7. Select a different Run while an Event is chosen: the choice is cleared.
8. Scroll a long list; check the detail island scrolls when a section is long.

**2026-10-31, at the user's request (screenshot `I:\Screenshot 2026-10-31 122259.png`):** above the Events list, two rows of filter buttons: `Day: All D1 ... Dn` (1 to the Run's latest day) and `Hour: All H1 ... H6` (with `H0` first when the Run has Events in hour 0). A day and an hour can be combined. An Event with no day or hour of its own counts in the last one before it. Times stay from the start of the Run; the day/hour column marks its first visible row. The filter is per Run and is cleared when another Run is selected (or a new one goes live); it does not move the board or the day buttons in the header. View model `events::{Filter, days, hours, rows}`, state in `Session::filter`, tests in `events_tests.rs` and `session_tests.rs`.
