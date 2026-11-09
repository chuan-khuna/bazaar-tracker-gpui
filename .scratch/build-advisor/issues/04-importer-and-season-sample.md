# 04: Importer: Scraped runs into imported_*, stats, and the season sample

**What to build:** `just advisor import` reads the run scraper's files into `imported_runs`, `imported_run_cards`, `imported_picks` and `imported_combats`, then recomputes the stats tables. `knowledge/` gets the season sample and the pair and pick queries. It's plain Python with no model. See the spec's Importer and Season sample decisions, and the record's Synergy section.

**Blocked by:** 03

**Status:** resolved

- [x] Parses `.bazaar-tracker/raw_data/scraper/builds/*/runs/season-NN/*.json` (`format: 1`): the run's fields, `summary.board` and skills (card id, tier, slot, size, enchantment), the timeline's `selection` events (offered, taken), and `summary.combats`.
- [x] Re-importing a run replaces its rows, so running the import twice changes nothing.
- [x] Stats per hero and season: support, lift, win-rate gain with a Beta prior, adjacency from slot and size, and pick-after-pick. Pairs under the minimum support are dropped.
- [x] `knowledge.sample(hero, min_runs=10)` returns the current season, plus the previous one when the current one is thin, along with the seasons used and their run counts.
- [x] Queries return `n`, the seasons, and the run ids behind each count.
- [x] pytest: a fixture reproduces the record's example numbers (lift 2.25 and 2.81, picks 15 → 7); the import is idempotent; the season fallback works; a card id that isn't found is reported, not silently dropped.
- [x] Documents: `advisor.md` (the importer, the tables, the formulas). `scrapers.md` and `scraping-manual.html` say how to import after scraping.

## Comments

Done (2026-11-08):

- `apps/advisor/src/importer/`: `scraped.py` (one file into rows, pure), `importer.py` (`import_builds`, `recompute_stats`, one transaction), `cli.py` (the `advisor-import` script). `just advisor import [--db PATH] [--builds DIR] [--min-support N] [--game-dir DIR]`.
- `apps/advisor/src/knowledge/`: `stats.py` (the formulas), `runs.py` (imported runs loaded as `RunData`), `sample.py` (`sample`, `current_season`), `queries.py` (`card_runs`, `together`, `partners`, `picks`, `picked_after`), re-exported from `knowledge`.
- Tests: `tests/test_importer.py`, `tests/test_knowledge.py`, and `tests/scraped_runs.py`, which writes run files and the record's 15 Jules runs as a fixture. 70 tests pass.
- Documents: `advisor.md` (the importer, what goes where, the statistics, the sample and queries, the tests), `scrapers.md` and `scraping-manual.html` (importing after a scrape), `history-db.md` (how the stats tables are filled, `size` in slots), `data-folder.md`, `architecture.md`, `INDEX.md`. The manual's stale `cd app\scraper` is now `apps\scraper`.

Done differently, or decided here:

- **Signatures.** `knowledge.sample(connection, hero, min_runs=10, season=None)` takes a SQLAlchemy connection first; so do the queries. The queries take card ids (name lookups are the tools' job, ticket 05).
- **Minimum support is 3**, not the record's example of 5: with 5 the record's own example pairs (n = 3) would be dropped. 3 matches the record's table exactly (lift shown for the n = 3 pairs, none for n ≤ 2). `--min-support` and the `min_support` arguments change it.
- **The Beta prior** is worth 10 games (about one run) at the sample's own win rate, on combats (wins, losses), not a fixed α = β. Win-rate gain is `rate(A and B) − rate(A without B)`, None when every run with A has B.
- **Pair and adjacency rows are stored both ways round** (`n` and `lift` symmetric, `win_gain` directional); a pick row means `b` was taken after `a` was first taken.
- **The queries compute from the imported rows** over the season sample, with the same functions as the stats tables, because a sample can span two seasons and the tables hold one each. The stats tables are still recomputed on every import, for SQL inspection and later use.
- **Picks:** one row per option; the same card offered twice in one event is two rows with one taken; a `selectedCard` missing from the options (42 encounters and encounter steps in today's data) is added as a taken row. All card kinds are kept (items, skills, encounters), so the pick-after-pick table is large (13,803 rows for Jules S20 from 15 runs). Filtering it to items and skills would need the card type, which `imported_picks` doesn't store; left as is.
- **Current season** is the newest season of any imported run, so a hero with no runs this season still starts from it and falls back to the previous one. Runs with no season (`season-unknown`) are imported but are in no sample and no stats.
- Unknown card ids are checked against `GameData.db` for Board, skills and picks, reported with the file's title and run count, and imported anyway. With no `GameData.db` the report says the ids weren't checked.

Verification:

- `just advisor test`: 70 passed.
- The repo's `.bazaar-tracker/history.db` is still at version 8, so `just advisor import` refuses it ("schema version 8 … Start the tracker once"). It was not migrated. Against a copy migrated to 9 with the tests' helper: `just advisor import --db <copy> --builds .bazaar-tracker/raw_data/scraper/builds` imported 16 runs from 16 files in about a second, Jules S20 28 pairs / 0 adjacent / 13,803 pick-after-pick, The Dragons S20 none, every card id found in `GameData.db`. On the real data the queries give Laurel's Bouncy Ball 4/15 runs, lift 2.25 with Giant Lollipop and 2.81 with Pizza Cutter, picks 15 → 7, as the record says. Decanter and Cheese Wheel show 17 offers today (the record says 16 and 15) and Cheese Wheel is never with Giant Lollipop (record: 1): the record's numbers were taken slightly differently or from an earlier scrape; the fixture reproduces the record's figures exactly.
- No Rust changed, so `cargo test --workspace` was not rerun.
- By hand once the tracker has migrated the real file: `just advisor import`, then run it again and see the same counts.
