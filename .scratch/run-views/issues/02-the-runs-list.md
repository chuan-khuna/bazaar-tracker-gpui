# 02: The runs list in the sidebar

**What to build:** The sidebar lists the Runs in `history.db`, newest first, and updates live as Events are saved; clicking a row selects the Run. See the spec's Reading history, Runs list and Selection decisions.

**Blocked by:** 01

**Status:** ready-for-human

- [x] `RunList` loads every Run from `history.db` at start-up, newest first.
- [x] A saved Event re-reads its Run's row and inserts or replaces it, keeping the order; a new Run is on top.
- [x] A `RunRebound` leaves one row under the new id.
- [x] The live Run id is the Run of the latest saved Event, cleared by a saved `RunEnded` for it or by losing the game, and renamed by a rebinding.
- [x] Each row shows the start date and time (local), the LIVE chip on the live Run, the hero, the record (`7W-2L`, win or loss colour once there is an outcome) and `Day N`.
- [x] Clicking a row selects the Run (highlighted); a Run that becomes live is selected for the player; a rebinding keeps the selection.
- [x] The sidebar keeps its empty state while there are no Runs, and scrolls when there are many.
- [x] Tests (runs seam): against a temporary `History`: newest first, upsert order, new Run on top, rebinding, live Run tracking and clearing, selection rules.
- [x] By-hand checks are listed under `## Comments`.

## Comments

Implemented by an agent; build, `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean. What is left is seeing it against the game.

- `tracking::session::Session` holds the Runs (newest first), the live Run id and the selected Run id. `Session::load(&History)` reads the list once; `Session::apply(&Notice, &History)` re-reads the affected Run (`get_run`) after each `EventSaved` or `RunRebound` and puts it in place, so the summary rules stay in the history store. `Session::rows()` gives the sidebar's `RunRow`s. `tracking::when::short` formats the start time in local time (`chrono`, added as a dependency with only the `clock` feature).
- `tracking::service::Link` (new) is what the window holds of the service: the notices and `Option<Arc<History>>`. `TrackerApp::new` now takes it instead of the loose parameters of ticket 01; the window and the tracker share one `Arc<History>`. A history that cannot be listed becomes a sticky error and an empty list.
- Where the spec was silent: a Run that sits in the list but has no hero shows "Unknown hero"; a Run with no victories or losses yet shows `0W–0L`; the record is coloured only when the Run has a Win or Loss outcome, otherwise it is drawn in the subtle colour; `Outcome::Unknown` counts as no outcome. A Run keeps its place by its start time, not by its latest Event.
- `Session::runs/live/selected` are `#[cfg(test)]` for now because only the tests read them; the next ticket makes the ones it needs ordinary methods. The unused time formats were left out until a ticket uses them.
- Not built, as the spec says: delete and refresh buttons.

By hand, with `just run` and the game (the first run may need a few Events recorded before it shows anything; `just track` populates the same `.bazaar-tracker/history.db`):

1. With an existing `.bazaar-tracker/history.db` from earlier `just track` sessions, start the app: the sidebar lists those Runs newest first, each with start time, hero, record and day. Times are in your local time zone. The empty-state text shows only when there are none.
2. Start a new Run in the game while the app is open: a new row appears at the top within a moment, carries a LIVE chip, is highlighted (selected), and its record and day change as you play (the first Event may need a Choice to be made).
3. Finish or lose the Run: the LIVE chip goes away and the record turns green (win) or red (loss).
4. Click another row: it highlights and the first goes back to normal. Scroll the list with many Runs.
5. Close the game mid-run: the LIVE chip goes away.
6. If the first Events of a new Run were saved under a `local-` id, check that the Run is one row after its real id is known (look at `[rebound]` lines from `just track` in an earlier session for when this happens).
