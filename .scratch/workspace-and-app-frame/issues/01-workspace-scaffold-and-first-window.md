# 01: Workspace scaffold and first window

**What to build:** A developer runs `just run` and a native "Bazaar Tracker" window opens in Rosé Pine with wordle-gpui's frame: a flat custom title bar holding only the title and a ☰ button, three island panels (runs sidebar, main run panel, detail panel) showing their empty states, and a flat status bar. The repo becomes the agreed workspace: the root package is the app, and `unityspy-rs` and `bazaar-tracker` exist as compiling module skeletons. See the spec (`../spec.md`), the architecture document and the 2026-10-31 UI mockup.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The root manifest is both the `bazaar-tracker-gpui` package and a workspace with members `crates/*`; edition 2024.
- [ ] `unityspy-rs` is Windows-only at crate level and has `mem` (process, reader with the `Memory` trait, module) and `mono` (pe, domain, offsets, class, field, statics, object, value, bcl) as documented skeletons.
- [ ] `unityspy-rs` has a `dump-classes` example that builds and prints that it is not implemented yet.
- [ ] `bazaar-tracker` has `log`, `memory`, `run`, `enums`, `history`, `catalog`, `upload` as documented skeletons and an empty recorded-logs fixtures folder; it does not depend on gpui.
- [ ] A `justfile` with `run`, `test` (`cargo test --workspace`), `spike` and `release`, using PowerShell as the Windows shell.
- [ ] The gpui stack is pinned exactly as in wordle-gpui (`gpui-pre =0.3.9` as `gpui`, `gpui-pre-platform`, `gpui-base =0.7.3`, `gpui-kit-assets 0.7.3`, `gpui-component 0.7.3` without default features).
- [ ] The app has the `shell`, `overlays`, `theme` and `ui` layers; nothing outside `ui` draws.
- [ ] The `Theme` struct has every role from the spec (wordle's generic roles plus outcome, tier and badge roles); only Rosé Pine is filled in.
- [ ] The window opens centred at 1000×640, minimum 820×520, titled "Bazaar Tracker".
- [ ] The title bar's minimise, maximise and close work; dragging the title bar moves the window; clicks on its buttons are not taken as drags.
- [ ] The sidebar shows "No runs yet — start a run in The Bazaar and it will appear here."; the main panel asks to start or select a run; the detail panel asks to select an event.
- [ ] The status bar shows a "Not tracking" connection indicator, an error slot that stays until dismissed with ✕, and a notice slot that hides after 2 seconds.
- [ ] Ctrl+Q and closing the window quit the app.
- [ ] `just test` passes and the library crates compile with no warnings.
