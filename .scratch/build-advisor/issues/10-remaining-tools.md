# 10: The remaining tools: shop_plan, arrangement, similar_runs

**What to build:** Add three tools and the prompt rules for when to use each:
- `shop_plan`: the tier odds for the day, the item odds for wanted cards, and the encounters strong runs chose that day.
- `arrangement`: the usual adjacent pairs and positions, and the descriptions that mention position.
- `similar_runs`: the Scraped runs whose final Board overlaps yours most.

See the record's Tools table.

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] `shop_plan` reads `imported_shop_odds`, `imported_item_odds`, and encounter picks from `imported_picks`.
- [ ] `arrangement` reads `imported_adjacency_stats` and slot positions, and returns the descriptions that mention adjacency, left or right.
- [ ] `similar_runs` ranks by Board overlap and returns each run's result, Board, skills and URL.
- [ ] Each tool returns `n` and the seasons, with pytest on fixtures.
- [ ] Documents: `advisor.md` (the tools).

## Comments
