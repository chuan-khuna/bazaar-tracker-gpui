# 14: A clearer Run delete confirmation in the sidebar

**What to build:** Asked for by the user on 2026-11-08 (screenshot): the sidebar's Run delete confirmation needs a better button and a better place for its message. Today the first click turns the row's trash icon into a small red "Delete?" pill squeezed beside the chat icon, and the note "Its Events and Conversations go too" replaces the row's date line in red (`apps/tracker/src/ui/frame/sidebar.rs`, added in ticket 02). The row's hero, record and day tags get crowded and the note wraps awkwardly.

**Blocked by:** none

**Status:** resolved

- [x] Confirming happens in a dedicated strip under the Run row (inside the same highlighted card, full width), not by replacing the date line. The row itself keeps its normal hero, tags and date.
- [x] The strip holds the message on its own line ("Delete this Run? Its Events and Advisor Conversations are deleted too.", or similar plain wording in the app's voice) and two clear buttons: a neutral **Cancel** and a red **Delete** (with the trash icon), sized like the app's other buttons, right-aligned.
- [x] The chat and trash icons are hidden while the strip is open, so nothing overlaps.
- [x] Esc or Cancel closes the strip; clicking another row closes it; only one row can be confirming at a time. Keep the existing two-step behaviour otherwise (nothing is deleted until Delete is clicked).
- [x] Use the theme's colours and the app's small border radius; reuse existing button styles/widgets (`ui/widgets.rs`) where possible. Apply the same pattern to the Advisor tab's Conversation ✕ → "Delete?" if it has the same problem, so the two look alike.
- [x] Unit tests for any state logic that can be tested without drawing (which row is confirming, Esc/Cancel/another click closing it).
- [x] Documents: `gpui-app.md` (sidebar delete, and the Conversation delete if changed).

## Comments

- 2026-11-08: Done. A pure `tracking::delete_ask::DeleteAsk` (tested in `delete_ask_tests.rs`) holds which row asks, for both the sidebar Runs (`TrackerApp::deleting_run`) and the Advisor Conversations (`AdvisorState::deleting`, now `ask_delete` / `cancel_delete` / `delete`). The trash (or ✕) opens `widgets::delete_strip` under the row inside its highlighted card: the message on its own line, then `text_button` Cancel and `delete_button` Delete (red, trash icon) right-aligned; the row's icons hide meanwhile. Escape (when no box takes the typing) closes either strip; clicking another row closes it. `data_button` moved from the settings overlay to `widgets::text_button`. `gpui-app.md` updated.
