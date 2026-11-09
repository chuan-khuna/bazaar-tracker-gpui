# 17: Collapsible Conversation list, 5 rows then scroll

**What to build:** Asked for by the user on 2026-11-08 (screenshot: a mark beside the "CONVERSATIONS +" header in the Advisor tab). The Conversation list can be collapsed, and when open it shows at most 5 rows and scrolls beyond that, so the messages keep most of the tab's height. Code: `apps/tracker/src/ui/advisor.rs`, `tracking/advisor/state.rs`.

**Blocked by:** none

**Status:** resolved

- [x] The "CONVERSATIONS" header gets a chevron (like the Run tab's collapsible "STASH" section) and toggles the list on click. The `+` button still works and doesn't toggle.
- [x] Collapsed, the header shows the open Conversation's title and `D5 H2` and the count (e.g. "CONVERSATIONS · 7 · Hello D14 H6"), so the user still knows which one is open.
- [x] Open, the list is at most 5 rows tall and scrolls (thin themed scrollbar, as the app's other scroll areas) when there are more. The selected row is scrolled into view when the list opens or the selection changes.
- [x] The collapsed/open state lasts while the app runs (across Runs and tab switches); saving it to settings is optional — pick one and document it.
- [x] Starting a new Conversation with `+` or a chat icon still works while collapsed.
- [x] Unit tests for the state (toggle, header summary text, visible-row cap logic if any).
- [x] Documents: `gpui-app.md` (the Conversation list).

## Comments

- 2026-11-08: Done. `AdvisorState` gains `list_collapsed` (kept across `open`/`forget`, not saved to settings: lasts while the app runs), `toggle_list`, `list_summary` (`7 · Hello D14 H6` folded, `7` open, "New conversation" while one is being started) and `scroll_target` (the shown row's index, none while folded). `ui/advisor.rs`: the header is clickable with `▾`/`▸` like STASH, `+` stops propagation; the open list is `LIST_ROWS` (5) rows of a fixed 26 px tall, scrolls with `widgets::scrollbar`, and scrolls the shown row into view once per open/selection change (`conversations_scrolled` in `TrackerApp`). Tests in `state_tests.rs`; `gpui-app.md` updated.
