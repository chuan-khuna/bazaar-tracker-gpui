# 08: A reply in full: thinking, tool lines, markdown, ask_user, follow-ups, usage, Stop

**What to build:** Draw every part of a reply in order (the thinking status, tool lines, text with light markdown, `ask_user`, `follow_ups` buttons, the usage line), and turn Send into Stop while a reply streams. The reply model is plain data built from events. See the spec's reply decisions. The behaviour to match is tau-app's `apps/chat` (`Message.tsx`, `DetailBlock.tsx`, `AskUserCard.tsx`, `ResponseStats.tsx`).

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] Thinking: `Thinking… Ns` while it streams, then a collapsed `Thought for Ns` that opens to the text.
- [ ] Tool lines: a spinner, then ✓ or ✕. Collapsed by default, they open to the arguments and the result as JSON. `suggest`, `follow_ups` and `ask_user` are drawn as their own components, not as lines.
- [ ] Light markdown: bold, italics, bullet and numbered lists, and inline code. Anything else stays plain text.
- [ ] `ask_user`: the question, its options (radio or checkbox), and a text box when `allowFreeText`. Answers are sent in tau-app's format. An answered card is locked, and is rebuilt from the next user message when history loads.
- [ ] `follow_ups`: 2–3 buttons, each sending its text.
- [ ] Usage line: model · time · tokens · cost · tools, with a tooltip showing input, output, cache read and write, cost and time.
- [ ] Send becomes Stop while a reply streams. Stop closes the stream and marks the reply "Stopped". A failed reply shows its error.
- [ ] Unit tests: the reply model from event sequences (normal, stopped, failed, tool error), the markdown splitter, and the ask_user answer format.
- [ ] Documents: `gpui-app.md` (how a reply is drawn).

## Comments
