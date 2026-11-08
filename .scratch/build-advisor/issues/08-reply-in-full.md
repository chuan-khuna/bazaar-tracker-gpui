# 08: A reply in full: thinking, tool lines, markdown, ask_user, follow-ups, usage, Stop

**What to build:** Draw every part of a reply in order (the thinking status, tool lines, text with light markdown, `ask_user`, `follow_ups` buttons, the usage line), and turn Send into Stop while a reply streams. The reply model is plain data built from events. See the spec's reply decisions. The behaviour to match is tau-app's `apps/chat` (`Message.tsx`, `DetailBlock.tsx`, `AskUserCard.tsx`, `ResponseStats.tsx`).

**Blocked by:** 07

**Status:** resolved

- [x] Thinking: `Thinking… Ns` while it streams, then a collapsed `Thought for Ns` that opens to the text.
- [x] Tool lines: a spinner, then ✓ or ✕. Collapsed by default, they open to the arguments and the result as JSON. `suggest`, `follow_ups` and `ask_user` are drawn as their own components, not as lines.
- [x] Light markdown: bold, italics, bullet and numbered lists, and inline code. Anything else stays plain text.
- [x] `ask_user`: the question, its options (radio or checkbox), and a text box when `allowFreeText`. Answers are sent in tau-app's format. An answered card is locked, and is rebuilt from the next user message when history loads.
- [x] `follow_ups`: 2–3 buttons, each sending its text.
- [x] Usage line: model · time · tokens · cost · tools, with a tooltip showing input, output, cache read and write, cost and time.
- [x] Send becomes Stop while a reply streams. Stop closes the stream and marks the reply "Stopped". A failed reply shows its error.
- [x] Unit tests: the reply model from event sequences (normal, stopped, failed, tool error), the markdown splitter, and the ask_user answer format.
- [x] Documents: `gpui-app.md` (how a reply is drawn).

## Comments

**Done.** The reply model is plain data in `apps/tracker/src/tracking/advisor/`, with no gpui:

- `reply.rs`: `Transcript` of `Turn::User` / `Turn::Reply`. A reply is the assistant messages (steps) and tool results after a question; each step's parts are its content blocks by index, as in tau-app's `useTauChat.ts`. The same folding builds it from streamed events and from saved messages (`from_saved` replays them as `message_end`s), so reloaded Conversations look as they did. It carries thinking timing, tool status and results, how the reply ended (`Done`, `Stopped`, `Failed`), the usage, the drawing order (`Reply::pieces`), the follow-ups and the question waiting for an answer.
- `markdown.rs` (light markdown into lines with styled ranges), `ask_user.rs` (parse, answer format, reading an answer back, the card's draft), `usage.rs` (stats, line, tooltip, tau-app's number formats).
- `conversation.rs` lost the old `Transcript`/`Entry`; `state.rs` gained Stop, `follow_up`, the `ask_user` draft and answer, and which thinking blocks and tool lines are open.
- Drawing in `ui/advisor.rs`; key routing for the card's text box in `ui/mod.rs`.
- Tests: `reply_tests.rs`, `markdown_tests.rs`, `ask_user_tests.rs`, `usage_tests.rs`, plus new cases in `state_tests.rs` and `client_tests.rs`.
- Docs: `gpui-app.md` (a reply, Stop, the client, keyboard, tests), `modules.md`, `INDEX.md`.

**Done differently:**

- **Stop uses a socket of its own.** `ureq` has no way to unblock a read from another thread, and the backend sends no keep-alives, so the reply's POST now goes over a hand-written HTTP/1.1 request on a `TcpStream`, with a small chunked reader. `Advisor` keeps a `StopHandle` per Conversation, and `Request::Stop` shuts the socket at once, so the backend cancels straight away. The other calls still use `ureq`. Send stays blocked until the stream has actually ended, to avoid a 409 race.
- **Thinking time after a reload.** The backend doesn't store how long thinking took, so a reloaded reply shows `Thought` with no seconds. Only a reply streamed in this session shows `Thought for Ns`.
- **Drawing order.** Thinking, tool lines and text are drawn in the order they came. `suggest` cards are moved after them, then `ask_user` cards, then follow-ups and usage, as the spec's reply decision says. tau-app draws strictly in block order.
- **Follow-up buttons** show only under the newest finished reply. Older ones would be stale because the situation has moved on.
- **A failed `ask_user`** is drawn as a tool line that shows its error. tau-app would still draw the card.
- **The usage tooltip** always shows the cache line, as the ticket lists it. tau-app shows it only when it isn't zero.
- **Stop while the Conversation is still being made** doesn't send the question; it goes back in the box.
- The `suggest` card is a placeholder (title, ranked cards with reasons, source line). The full card is ticket 09.

**Check by hand in the GUI** (only the models and the client have tests):

- Run `just advisor serve --fake`, then in the Advisor tab ask about a Run whose situation has an Offer. Check the order: tool line `rate_options` with ✓, then the text, then the placeholder card, the two follow-up buttons and the usage line, with the tooltip on hover.
- Open and close a tool line: arguments and result in Consolas.
- The spinner rotates. Watch `Thinking… Ns` count up with a real model that streams thinking (the fake sends none).
- Bold, italics, inline code and lists look right in every theme.
- Stop with a real model mid-reply: the button turns red, then back to Send once the stream closes. The reply reads `Stopped` and stays so after switching away and back.
- An `ask_user` card, single and multiple choice: picks, the text box (click, type, Enter, Escape, paste), Send, then locked; also answered from the main box. This needs a real model that calls `ask_user`, since the fake doesn't.
- A follow-up button sends its question.
- A failed reply shows its badge and error (e.g. a bad API key without `--fake`).

Already checked here: the real `Client` and `Transcript` against `advisor serve --fake` on a scratch v9 `history.db`. The streamed reply and the one rebuilt from saved messages matched (tool line, text, suggest card, follow-ups, usage), and the stream was chunked, as uvicorn sends it.
