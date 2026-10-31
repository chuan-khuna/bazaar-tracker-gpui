# 03: Panels: show, hide, resize, remember

**What to build:** The player can give the run more room. The runs sidebar shows and hides with Ctrl+B and the status-bar button, the detail panel with Ctrl+Alt+B and its status-bar button, and the sidebar can be resized by dragging its edge. The layout comes back the way it was left on the next launch. See the spec's user stories 24–34.

**Blocked by:** 02 (Data folder, settings, logging and single instance)

**Status:** resolved

- [x] Ctrl+B and the status bar's left button toggle the sidebar.
- [x] Ctrl+Alt+B and the status bar's right button toggle the detail panel.
- [x] Dragging the edge between sidebar and main panel resizes the sidebar, clamped to wordle-gpui's sidebar range.
- [x] The sidebar's open state and width, the detail panel's open state and the stash section's open state are saved under `[window]` and restored on launch.
- [x] Tests: the sidebar width clamp at both ends, through the settings seam.
