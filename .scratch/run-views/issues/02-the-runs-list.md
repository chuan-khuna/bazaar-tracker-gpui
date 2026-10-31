# 02: The runs list in the sidebar

**What to build:** The sidebar lists the Runs in `history.db`, newest first, and updates live as Events are saved; clicking a row selects the Run. See the spec's Reading history, Runs list and Selection decisions.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] `RunList` loads every Run from `history.db` at start-up, newest first.
- [ ] A saved Event re-reads its Run's row and inserts or replaces it, keeping the order; a new Run is on top.
- [ ] A `RunRebound` leaves one row under the new id.
- [ ] The live Run id is the Run of the latest saved Event, cleared by a saved `RunEnded` for it or by losing the game, and renamed by a rebinding.
- [ ] Each row shows the start date and time (local), the LIVE chip on the live Run, the hero, the record (`7W-2L`, win or loss colour once there is an outcome) and `Day N`.
- [ ] Clicking a row selects the Run (highlighted); a Run that becomes live is selected for the player; a rebinding keeps the selection.
- [ ] The sidebar keeps its empty state while there are no Runs, and scrolls when there are many.
- [ ] Tests (runs seam): against a temporary `History`: newest first, upsert order, new Run on top, rebinding, live Run tracking and clearing, selection rules.
- [ ] By-hand checks are listed under `## Comments`.

## Comments
