# 09: The recommendation card, with hover and expand

**What to build:** Draw a `suggest` tool result as the recommendation card. It has ranked rows with art, size, tier, fit, reason, and bars with `n`. Hovering a count shows its source runs with mini Boards, and clicking one expands it. A source line at the bottom carries the thin-data warning. See the spec's Recommendation card decision and the record's mock-up.

**Blocked by:** 08

**Status:** ready-for-agent

- [ ] Rows best first: art via `Names::art`, name, size, tier, ✓ fits or ✕ doesn't fit, the reason, and bars ("with your Board", "taken when offered"), each with its `n`.
- [ ] Hovering a count opens a popup listing its source runs, each with a mini Board drawn by the existing board renderer (`tracking::run::board`, `ui/run/board.rs`).
- [ ] Clicking a run expands it inside the card: its full Board, skills and result, and a link that opens its Bazaar DB page.
- [ ] Source line: runs, hero, seasons and profiles, with ⚠ thin under 10. Rows with no data show "no data · from card text".
- [ ] Unit tests: the card model from a `suggest` result (ranking, fits, thin, no data).
- [ ] By hand: the record's example renders like the mock-up.
- [ ] Documents: `gpui-app.md` (the recommendation card).

## Comments
