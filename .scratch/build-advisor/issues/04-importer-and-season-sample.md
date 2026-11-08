# 04: Importer: Scraped runs into imported_*, stats, and the season sample

**What to build:** `just advisor import` reads the run scraper's files into `imported_runs`, `imported_run_cards`, `imported_picks` and `imported_combats`, then recomputes the stats tables. `knowledge/` gets the season sample and the pair and pick queries. It's plain Python with no model. See the spec's Importer and Season sample decisions, and the record's Synergy section.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] Parses `.bazaar-tracker/raw_data/scraper/builds/*/runs/season-NN/*.json` (`format: 1`): the run's fields, `summary.board` and skills (card id, tier, slot, size, enchantment), the timeline's `selection` events (offered, taken), and `summary.combats`.
- [ ] Re-importing a run replaces its rows, so running the import twice changes nothing.
- [ ] Stats per hero and season: support, lift, win-rate gain with a Beta prior, adjacency from slot and size, and pick-after-pick. Pairs under the minimum support are dropped.
- [ ] `knowledge.sample(hero, min_runs=10)` returns the current season, plus the previous one when the current one is thin, along with the seasons used and their run counts.
- [ ] Queries return `n`, the seasons, and the run ids behind each count.
- [ ] pytest: a fixture reproduces the record's example numbers (lift 2.25 and 2.81, picks 15 → 7); the import is idempotent; the season fallback works; a card id that isn't found is reported, not silently dropped.
- [ ] Documents: `advisor.md` (the importer, the tables, the formulas). `scrapers.md` and `scraping-manual.html` say how to import after scraping.

## Comments
