# 15: Double-click an open thinking or tool block to collapse it

**What to build:** Asked for by the user on 2026-11-08 (screenshot). In the Advisor tab, an opened "Thought for Ns" block or tool line (e.g. a long `synergy` JSON result) can only be closed by its small chevron header, which may have scrolled far out of view. Double-clicking anywhere inside an open block's body should collapse it. Code: `apps/tracker/src/ui/advisor.rs`, open keys in `tracking/advisor/state.rs` (ticket 08).

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Double-clicking anywhere in an open thinking block's text, or an open tool line's arguments/result, collapses that block (same effect as clicking its header). A single click does nothing new, so text can still be clicked without collapsing.
- [ ] After collapsing, keep the collapsed header in view: if the header is above the visible area, scroll so it's visible (otherwise the user loses their place, since the block they were reading shrinks).
- [ ] Applies to thinking blocks and tool lines only; the recommendation card, `ask_user` card and plain text are unchanged.
- [ ] Optional, if cheap: a hint in the header's tooltip ("Double-click the text to collapse").
- [ ] Unit tests for the state change (double-click on an open key closes it; on a closed one does nothing).
- [ ] Documents: `gpui-app.md` (the reply's thinking and tool lines).

## Comments
