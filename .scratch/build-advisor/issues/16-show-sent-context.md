# 16: Show the context sent with each question, collapsible

**What to build:** Asked for by the user on 2026-11-08 (screenshots: a "> context" row under the user's question in the Advisor tab, and the Run tab's Snapshot with Board, Stash and skills). Under each user message, a collapsed **Context** row shows what the app sent to the Advisor with that question (the situation), so the user can check what the agent saw. Opened, it draws the situation the way the Run tab draws a Snapshot.

**Blocked by:** 11 (both change the backend's message store)

**Status:** ready-for-agent

- [ ] Backend: the situation sent with each message is saved with that user message (in `advisor_messages`, inside the existing JSON columns — no schema change) and returned by `GET /api/conversations/{id}/messages`, so a reloaded Conversation shows the same context. Check what is saved today first; add it only if missing. pytest for save and reload.
- [ ] App: under each user message, a collapsed row "Context · D14 H6 · 10/10 slots" (Day, Hour and a short summary). Collapsed by default; click (and double-click inside, as ticket 15 does) toggles it.
- [ ] Opened, it shows: hero, Day/Hour, gold, level, health, game state; the Board, Stash and skills drawn with the existing board renderer (`tracking::run::board`, `ui/run/board.rs`, compact size like the recommendation card's mini Board from ticket 09) with art, size, tier, enchantment; the Offer with each card's kind; the opponent if any.
- [ ] A small "JSON" toggle inside shows the raw situation exactly as sent, for debugging, with a copy button if ticket 12 added a reusable one.
- [ ] A message sent with no situation shows "No context sent".
- [ ] Unit tests: building the context view model from a saved situation (summary line, cards laid out, missing fields, no situation).
- [ ] Documents: `advisor.md` (the situation saved with messages), `gpui-app.md` (the Context row).

## Comments
