# 07: The Advisor tab: tabs, chat icons, Conversations, sending with the situation

**What to build:** In `apps/tracker`: the Run and Advisor tabs in the Detail island, the two chat icons, the Conversation list, the input box, and the HTTP+SSE client. The client sends each message with the current situation and shows the text as it streams. See the spec's The Advisor tab decisions.

**Blocked by:** 06

**Status:** ready-for-agent

- [ ] The Detail island has Run and Advisor tabs. The open tab survives selecting another Run, and isn't saved.
- [ ] A chat icon on sidebar Run rows (on hover and on the selected row), and one on the Main island beside the pop-out button (Live run only, greyed out otherwise). Each selects the Run and opens the Advisor tab on its newest Conversation.
- [ ] The Conversation list (title plus `D5 H2`, newest first, `+`, ✕ on hover with a confirmation) and the selected Conversation's messages, loaded from the API.
- [ ] The situation is built from `LiveView` for the Live run and from the last Snapshot for a finished Run, and sent with every message.
- [ ] A `ureq` client on a background thread parses SSE lines into events and sends them over an `async_channel`. The text appears as it streams.
- [ ] Enter sends and Shift+Enter starts a new line. When the backend can't be reached, the tab shows the message naming `just advisor serve`.
- [ ] Unit tests: the SSE parser, the situation from a `LiveView` and from a Snapshot, which Run and Conversation each icon opens, and the title format.
- [ ] By hand: ask during a Live run, and on a finished Run.
- [ ] Documents: `gpui-app.md` (the Detail island's tabs, the icons, the Advisor tab, the client thread) and `modules.md`.

## Comments
