# 12: Let the user copy `just advisor serve` from the Advisor tab

**What to build:** When the backend can't be reached, the Advisor tab shows "The Advisor isn't running. Start it with `just advisor serve`." (`apps/tracker/src/ui/advisor.rs`). Asked for by the user on 2026-11-08: make the command copyable.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] The command is drawn as inline code (monospace chip), not with literal backticks, with a copy button (icon) beside it that writes `just advisor serve` to the clipboard via gpui's clipboard API.
- [ ] After a click the icon briefly shows it copied (e.g. a check, or "Copied"), then returns.
- [ ] Reuse an existing copy button or icon in the app if there is one; otherwise add a small reusable one.
- [ ] A unit test where it can be tested without drawing (e.g. the text that gets copied).
- [ ] Documents: `gpui-app.md` (the Advisor tab's unreachable message).

## Comments
