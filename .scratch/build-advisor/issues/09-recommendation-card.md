# 09: The recommendation card, with hover and expand

**What to build:** Draw a `suggest` tool result as the recommendation card. It has ranked rows with art, size, tier, fit, reason, and bars with `n`. Hovering a count shows its source runs with mini Boards, and clicking one expands it. A source line at the bottom carries the thin-data warning. See the spec's Recommendation card decision and the record's mock-up.

**Blocked by:** 08

**Status:** resolved

- [x] Rows best first: art via `Names::art`, name, size, tier, ✓ fits or ✕ doesn't fit, the reason, and bars ("with your Board", "taken when offered"), each with its `n`.
- [x] Hovering a count opens a popup listing its source runs, each with a mini Board drawn by the existing board renderer (`tracking::run::board`, `ui/run/board.rs`).
- [x] Clicking a run expands it inside the card: its full Board, skills and result, and a link that opens its Bazaar DB page.
- [x] Source line: runs, hero, seasons and profiles, with ⚠ thin under 10. Rows with no data show "no data · from card text".
- [x] Unit tests: the card model from a `suggest` result (ranking, fits, thin, no data).
- [ ] By hand: the record's example renders like the mock-up.
- [x] Documents: `gpui-app.md` (the recommendation card).

## Comments

**Done (2026-11-08).** The card model is `tracking/advisor/recommendation.rs` (plain data, `recommendation_tests.rs`), replacing the placeholder `Suggestion` in `reply.rs`; `Piece::Suggest` is now `{ key, card: Recommendation }`. Drawing is a new `ui/recommendation.rs`; `ui/run/board.rs` gained `render_mini_grid` (the hover's mini Boards, from `tracking::run::board::board_grid`), `tier_colour` is shared, and `render_skills` takes any `&str` prefix. `AdvisorState` gained `hover` (which count's popup shows); expanded runs reuse its open-keys set, so they close when another Conversation is shown.

Choices made where the ticket left room:

- **"with your Board" bar:** one bar, the `with_board` pair with the highest lift (then the most runs), noted `(with Pizza Cutter · lift 2.81)`, matching the mock-up's single line (3/4, lift 2.8). The other pairs stay visible in the tool line's result. With an empty Board the bar is "on final Boards" (`runs_with`) instead.
- **"taken when offered" hover:** lists the runs it was offered in (each once), with a `taken` badge where it was taken.
- **Expanded runs** sit under the row whose bar listed them; a second click or ✕ closes one. The Bazaar DB link opens only `https://bazaardb.gg/` URLs, via `cx.open_url` (read-only; no upload).
- **Thin:** the result's `thin`, else `n` under 10 when `n` is present; ⚠ is appended to the source line if the backend's text lacks it, and the line is drawn in `gold` with a warning icon.
- `Fit` variants are `Yes` / `No` / `Unknown` (clippy rejects `DoesNotFit` inside `Fit`).

Verification: `cargo build` clean; `cargo test --workspace` passes (415 in the app, 11 new card tests plus updated reply tests); clippy shows only the 6 known warnings. No Python changed.

**Not done: by hand, the record's example renders like the mock-up.** This needs the GUI (the app against `just advisor serve --fake` with a v9 `history.db` holding imported Jules runs). Things to look at then: the popup's position and that the pointer can move from the count into it without it closing (it overlaps the count by 2 px for this), the popup scrolling with many offered runs, art box sizes, and the expanded Board's width inside the detail island.
