# 04: Menu, command palette and settings overlay

**What to build:** Everything the app can do is reachable from the ☰ menu and from a Ctrl+P command palette that lists the same commands. Settings… opens a settings overlay (empty for now). Overlays are plain state structs, copied and adapted from wordle-gpui, that take a key and return an outcome. See the spec's Overlays and Search decisions.

**Blocked by:** 03 (Panels: show, hide, resize, remember)

**Status:** resolved

- [x] ☰ opens the menu; its rows are Settings…, Show sidebar, Show detail, Show run duration, Reset layout and Quit.
- [x] Each row shows its on-dot, current value or shortcut drawn as gpui-kit `Kbd` keys in Windows notation; the menu's last line shows the palette shortcut.
- [x] Show run duration toggles and is saved, with no visible effect yet.
- [x] Reset layout restores the default panel layout and window size.
- [x] Ctrl+P opens the palette with the same commands; typing filters with wordle-gpui's loose matching; Up/Down move; Enter or a click runs; Ctrl+P, Escape or a click outside closes.
- [x] While the menu or palette is open, typing does not reach the rest of the window.
- [x] Only one overlay is open at a time; opening one closes the others.
- [x] Settings… opens a settings overlay that closes with Escape, ✕ or a click outside.
- [x] Tests (overlays seam): palette filtering, movement, Enter runs the highlighted command, Escape closes; the menu and palette offer the same commands; search matches as in wordle-gpui's tests.
