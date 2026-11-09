# 18: Pick an Event to attach to a new chat's context

**What to build:** Asked for by the user on 2026-11-08 (screenshot: the Run tab's Event table with a mark on the right of an "Options · 1 option · Encounter" row). The user can pick an Event in the Run tab's Event table and start an Advisor chat about it: the situation sent is the game state at that Event (its Snapshot: Board, Stash, skills, gold, health, level, Day/Hour) plus the Event itself (e.g. the Options offered, what was Selected or Sold), instead of the Run's latest state.

**Blocked by:** 16 (the Context row shows what was attached)

**Status:** resolved

- [x] Event rows in the Run tab's Event table get a chat icon on the right (on hover, and on the selected row), matching the chat icons from ticket 07. Clicking it opens the Advisor tab on a new Conversation with that Event attached.
- [x] The input box shows the attachment as a removable chip above the text ("Event #7 · D5 H2 · Options: 3 options · Choice ✕"). Removing it goes back to the current situation (Live or last Snapshot, as today).
- [x] The situation sent is built from the Snapshot at that Event (reuse what the Run tab's SNAPSHOT section shows for the selected Event), and when the Event is an Options/Choice, its offered cards become the situation's Offer (with each card's kind) and, if picked, what was picked. Selected/Sold Events say what was taken or sold. Extend the situation schema in `apps/advisor/src/agents/advisor/situation.py` only if needed (e.g. an `event` field with id, kind, time, detail), validating it, and render it in `prepare()`'s prompt.
- [x] The attachment applies to the message it was sent with; later messages in the same Conversation use the current situation unless an Event is attached again. The Context row (ticket 16) shows it.
- [x] The Conversation's `D5 H2` comes from the attached Event's Day and Hour.
- [x] Tests: Rust unit tests for building the situation from an Event + Snapshot (Options → Offer, picked, Sold), the chip text, and which Conversation the icon opens; pytest for any schema change.
- [x] Documents: `gpui-app.md` (Event chat icon, the attachment chip), `advisor.md` (the situation's event field, if added).

## Comments

- 2026-11-08 (agent): Done. Event rows get a chat icon (hover / selected row; `ui/run/events.rs`) that calls `TrackerApp::open_advisor_event` → `AdvisorState::open_event` (new Conversation, Event attached). Chip text in `tracking/advisor/attachment.rs` (`chip_text`), drawn above the text in the box with a ✕ (`detach`). `situation::for_event` builds the situation from the Snapshot at the Event (Day/Hour from the Event, so the Conversation's `D5 H2` follows), Options → Offer with kinds, and a new `event` field (`id`, `number`, `kind`, `time`, `detail`, `picked`, `sold`), validated and rendered first in `prepare()`'s Situation section (`apps/advisor/src/agents/advisor/situation.py`). The attachment is taken by the message it is sent with and given back if that message never reaches the backend. The Context row shows `Event #7` in its summary and Event/Picked/Sold facts. Tests in `attachment_tests.rs`, `situation_tests.rs`, `state_tests.rs`, `context_tests.rs`, `tests/test_situation.py`; docs in `gpui-app.md` and `advisor.md`.
