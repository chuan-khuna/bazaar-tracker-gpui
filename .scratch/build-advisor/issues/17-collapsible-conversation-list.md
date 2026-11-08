# 17: Collapsible Conversation list, 5 rows then scroll

**What to build:** Asked for by the user on 2026-11-08 (screenshot: a mark beside the "CONVERSATIONS +" header in the Advisor tab). The Conversation list can be collapsed, and when open it shows at most 5 rows and scrolls beyond that, so the messages keep most of the tab's height. Code: `apps/tracker/src/ui/advisor.rs`, `tracking/advisor/state.rs`.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] The "CONVERSATIONS" header gets a chevron (like the Run tab's collapsible "STASH" section) and toggles the list on click. The `+` button still works and doesn't toggle.
- [ ] Collapsed, the header shows the open Conversation's title and `D5 H2` and the count (e.g. "CONVERSATIONS · 7 · Hello D14 H6"), so the user still knows which one is open.
- [ ] Open, the list is at most 5 rows tall and scrolls (thin themed scrollbar, as the app's other scroll areas) when there are more. The selected row is scrolled into view when the list opens or the selection changes.
- [ ] The collapsed/open state lasts while the app runs (across Runs and tab switches); saving it to settings is optional — pick one and document it.
- [ ] Starting a new Conversation with `+` or a chat icon still works while collapsed.
- [ ] Unit tests for the state (toggle, header summary text, visible-row cap logic if any).
- [ ] Documents: `gpui-app.md` (the Conversation list).

## Comments
