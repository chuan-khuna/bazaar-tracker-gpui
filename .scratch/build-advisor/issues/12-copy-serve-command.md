# 12: Let the user copy `just advisor serve` from the Advisor tab

**What to build:** When the backend can't be reached, the Advisor tab shows "The Advisor isn't running. Start it with `just advisor serve`." (`apps/tracker/src/ui/advisor.rs`). Asked for by the user on 2026-11-08: make the command copyable.

**Blocked by:** none

**Status:** resolved

- [x] The command is drawn as inline code (monospace chip), not with literal backticks, with a copy button (icon) beside it that writes `just advisor serve` to the clipboard via gpui's clipboard API.
- [x] After a click the icon briefly shows it copied (e.g. a check, or "Copied"), then returns.
- [x] Reuse an existing copy button or icon in the app if there is one; otherwise add a small reusable one.
- [x] A unit test where it can be tested without drawing (e.g. the text that gets copied).
- [x] Documents: `gpui-app.md` (the Advisor tab's unreachable message).

## Comments

- 2026-11-08: Done. `NOT_RUNNING` no longer holds the command; the new `SERVE_COMMAND` holds it, and `AdvisorState::status_command()` returns it while the backend is unreachable. The app had no copy button, so I added two reusable widgets in `ui/widgets.rs`: `inline_code` (a Consolas chip) and `copy_button` (a Copy icon that turns into a success-coloured Check). `MONO` now lives in `ui/widgets.rs`, and `ui/advisor.rs` imports it from there. `TrackerApp::copy_text` writes the text with `cx.write_to_clipboard` and shows the tick for `COPIED_TIME` (1.5 s). The tooltip reads Copy, or Copied while the tick shows. `state_tests.rs` checks the copied text, that `NOT_RUNNING` has no backticks, and that there is no command when the backend is ready or unavailable.
