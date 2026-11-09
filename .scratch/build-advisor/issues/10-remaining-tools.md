# 10: The remaining tools: shop_plan, arrangement, similar_runs

**What to build:** Add three tools and the prompt rules for when to use each:
- `shop_plan`: the tier odds for the day, the item odds for wanted cards, and the encounters strong runs chose that day.
- `arrangement`: the usual adjacent pairs and positions, and the descriptions that mention position.
- `similar_runs`: the Scraped runs whose final Board overlaps yours most.

See the record's Tools table.

**Blocked by:** 05

**Status:** resolved

- [x] `shop_plan` reads `imported_shop_odds`, `imported_item_odds`, and encounter picks from `imported_picks`.
- [x] `arrangement` reads `imported_adjacency_stats` and slot positions, and returns the descriptions that mention adjacency, left or right.
- [x] `similar_runs` ranks by Board overlap and returns each run's result, Board, skills and URL.
- [x] Each tool returns `n` and the seasons, with pytest on fixtures.
- [x] Documents: `advisor.md` (the tools).

## Comments

Done (2026-11-08): `shop_plan`, `arrangement` and `similar_runs` in `apps/advisor/src/agents/advisor/tools/`, registered in `tools_for` and `TOOL_NAMES` (before `follow_ups` and `ask_user`), with three prompt rules for when to use each. The numbers come from two new `knowledge/` modules: `odds.py` (the saved shop and item odds; an item's `body` read as `bazaardb::item_odds::parse_json` reads it, including the `N+` column and unmeasured days) and `boards.py` (`day_picks`, `arrangement`, `similar_runs`). Each tool returns `n`, the seasons, `sample` and `thin` from the hero's season sample, plus the run ids behind each count, and raises `ValueError` on bad arguments. Tests in `tests/test_plan_tools.py` (9 tests; 131 pass). `advisor.md` (layout, queries, tools table, prompt rules, tests) and `INDEX.md` updated.

Done differently or worth a look:
- `arrangement` doesn't read the `imported_adjacency_stats` table: it works adjacency out from the imported rows with the same formula (`stats.adjacency_stats`), like the other queries, because a season sample can span two seasons and the table holds one season per row.
- "Strong runs" for `shop_plan`'s encounters means runs with 10+ wins (`knowledge.STRONG_WINS`). Encounters are told apart by the catalog's card type ending in `Encounter` (merchants, events, fights; `EncounterStep` left out). With an empty catalog there are no encounter picks, and a note says why.
- Item odds are for the tier the player owns the card at (situation Board, then Stash), else `None` (not owned), falling back to any saved tier for that hero. Odds are given as percent (`*_pct`). With no shop odds saved for the day, `tier_odds` is null with a note; a day after the last saved one uses that last day.
- `similar_runs` ranks by shared cards on the final Board (skills not counted), then Jaccard, then wins; the model reads the whole result (no `model_view`), since the runs are what it answers from. Nothing new is drawn in the app: these show as tool lines.
