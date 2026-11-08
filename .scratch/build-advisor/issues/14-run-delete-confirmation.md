# 14: A clearer Run delete confirmation in the sidebar

**What to build:** Asked for by the user on 2026-11-08 (screenshot): the sidebar's Run delete confirmation needs a better button and a better place for its message. Today the first click turns the row's trash icon into a small red "Delete?" pill squeezed beside the chat icon, and the note "Its Events and Conversations go too" replaces the row's date line in red (`apps/tracker/src/ui/frame/sidebar.rs`, added in ticket 02). The row's hero, record and day tags get crowded and the note wraps awkwardly.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Confirming happens in a dedicated strip under the Run row (inside the same highlighted card, full width), not by replacing the date line. The row itself keeps its normal hero, tags and date.
- [ ] The strip holds the message on its own line ("Delete this Run? Its Events and Advisor Conversations are deleted too.", or similar plain wording in the app's voice) and two clear buttons: a neutral **Cancel** and a red **Delete** (with the trash icon), sized like the app's other buttons, right-aligned.
- [ ] The chat and trash icons are hidden while the strip is open, so nothing overlaps.
- [ ] Esc or Cancel closes the strip; clicking another row closes it; only one row can be confirming at a time. Keep the existing two-step behaviour otherwise (nothing is deleted until Delete is clicked).
- [ ] Use the theme's colours and the app's small border radius; reuse existing button styles/widgets (`ui/widgets.rs`) where possible. Apply the same pattern to the Advisor tab's Conversation ✕ → "Delete?" if it has the same problem, so the two look alike.
- [ ] Unit tests for any state logic that can be tested without drawing (which row is confirming, Esc/Cancel/another click closing it).
- [ ] Documents: `gpui-app.md` (sidebar delete, and the Conversation delete if changed).

## Comments
