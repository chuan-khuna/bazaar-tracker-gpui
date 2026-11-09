# 07: The Advisor tab: tabs, chat icons, Conversations, sending with the situation

**What to build:** In `apps/tracker`: the Run and Advisor tabs in the Detail island, the two chat icons, the Conversation list, the input box, and the HTTP+SSE client. The client sends each message with the current situation and shows the text as it streams. See the spec's The Advisor tab decisions.

**Blocked by:** 06

**Status:** resolved

- [x] The Detail island has Run and Advisor tabs. The open tab survives selecting another Run, and isn't saved.
- [x] A chat icon on sidebar Run rows (on hover and on the selected row), and one on the Main island beside the pop-out button (Live run only, greyed out otherwise). Each selects the Run and opens the Advisor tab on its newest Conversation.
- [x] The Conversation list (title plus `D5 H2`, newest first, `+`, ✕ on hover with a confirmation) and the selected Conversation's messages, loaded from the API.
- [x] The situation is built from `LiveView` for the Live run and from the last Snapshot for a finished Run, and sent with every message.
- [x] A `ureq` client on a background thread parses SSE lines into events and sends them over an `async_channel`. The text appears as it streams.
- [x] Enter sends and Shift+Enter starts a new line. When the backend can't be reached, the tab shows the message naming `just advisor serve`.
- [x] Unit tests: the SSE parser, the situation from a `LiveView` and from a Snapshot, which Run and Conversation each icon opens, and the title format.
- [ ] By hand: ask during a Live run, and on a finished Run.
- [x] Documents: `gpui-app.md` (the Detail island's tabs, the icons, the Advisor tab, the client thread) and `modules.md`.

## Comments

Done on `feat/item-odds-tier` (not committed).

- **Built (no gpui):** `apps/tracker/src/tracking/advisor/`: `sse.rs` (the line parser: pieces, CRLF, several `data:` lines, comments, the name from `event:` else the JSON's `type`; frames into `AdvisorEvent`), `situation.rs` (`for_run`, `from_board_state`, `from_snapshot`, serialised in the schema's fields only), `conversation.rs` (`Conversation` with `time()`/`label()` giving `D5 H2`, newest first, and a minimal `Transcript`: user and assistant text, the tools a message called, errors), `state.rs` (`AdvisorState`: `DetailTab`, `ChatIcon`/`icon_run`, the followed Run, its Conversations, the shown one, the box, the backend's state; every method returns the `Request`s to run), `client.rs` (`Client` on `ureq` 3: list, create, delete, messages, send with the reply read line by line; `Advisor::ask` runs each request on a short-lived `advisor-client` thread and sends `Update`s over an `async_channel`).
- **Built (drawing):** `ui/advisor.rs` (the tabs, the Conversation list with `+` and ✕ → `Delete?`, the messages with `Thinking…` and the text as it streams, the multi-line box, both chat icons), wired in `ui/mod.rs` (fields, the update loop, rebound/delete/clear, key routing), `ui/run/detail.rs` (tabs on top), `ui/frame/sidebar.rs` (row icon), `ui/live.rs` and `ui/run/board.rs` (icon beside the pop-out button, in the placeholder and the Live board section). `LiveState::board()` and `live::state_words` made reachable.
- **Tests:** 42 new (`tracking/advisor/*_tests.rs`): the SSE parser, the situation from a board read and from a Snapshot (and the fallbacks), titles and newest first, the transcript, which Run and Conversation each icon opens, sending to a new or the shown Conversation, not running and trying again, 503, delete, rebound and forget; and the client against a one-shot local socket (listing with an encoded run id, an error's detail, a streamed reply, nothing listening → `Unreachable`).
- **Checked against the real backend:** `just advisor serve --fake` with `BAZAAR_TRACKER_DATA_DIR` on a scratch folder (a fresh v9 `history.db` with one Run made by `History::open`; the repo's `.bazaar-tracker/history.db` untouched). A throwaway ignored test drove `AdvisorState` + `client::run`: list (empty) → create (title "Which of these should I buy?", D5 H2 from the situation) → send → `message_start`/`delta`/`message_end`/`stats`/`done` parsed, transcript user + assistant text; reloading `GET …/messages` gave the same transcript; the situation was accepted by the schema. The throwaway test was removed and the server stopped.
- **Not verified by hand in the GUI:** I couldn't click through the window, so the layout, the icons' hover/selected visibility, the greyed Live icon, typing/Enter/Shift+Enter/Ctrl+V in the box, and asking during a Live run and on a finished Run against the game are unchecked (the "By hand" box is left open).
- **Done differently:**
  - The Live situation is built from the board read the `LiveView` is drawn from (`LiveState::board()`), not from `LiveView` itself: `LiveView` holds formatted strings and grids without the raw numbers (gold, health) the schema wants. Board slots and sizes come from the same `board_grid` placement the board draws, so they match what is shown.
  - The Live run before any board read (or with the game detached) falls back to its last Snapshot, like a finished Run. A finished Run sends `game_state: "Run over"`.
  - Offer kinds: `Skill` → `skill`; an item in `Encounter` (shop) → `merchant_item`; an item elsewhere → `loot`; anything else → `encounter_choice`. An offered id with no card in the read is left out (`name` is required).
  - The messages view is deliberately minimal (text, "Used <tools>", errors, `Thinking…`); thinking, tool lines, markdown, the card, follow-ups, usage and Stop are ticket 08/09. There is no Stop yet: a stream ends only when the backend ends it.
  - Deleting the shown Conversation shows the newest one left. A message that never reached the backend (unreachable or an error status) goes back into the box.
  - The text box is hand-rolled like the odds search (append-only editing: Backspace deletes the last character, no cursor movement or selection); Ctrl+V/Cmd+V pastes.
- **Documents:** `gpui-app.md` (new "Advisor tab" section: tabs, icons, list, messages, box, situation table, trying again, following the Run, the client thread and its diagram; startup table, sidebar, Live section, keyboard, dependencies (`ureq`, icons), tests), `modules.md` (diagram, the five `tracking/advisor/*` rows, `ui/advisor`, sidebar, detail, mod), `advisor.md` (what exists so far), `INDEX.md` (gpui app summary).
