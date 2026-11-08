# Advisor

The Build Advisor's service in `apps/advisor/`: a Python agent the player will ask about the Run they are playing, from the tracker's Advisor tab. It answers from Scraped runs and the card catalog, through tools that compute the numbers; the model never makes a number up. The plan and its reasons are in the [2026-11-08.001 record](../records/2026-11-08.001-build-advisor-agent.html) and `.scratch/build-advisor/spec.md`. Terms such as Run, Offer, Board and Scraped run are as in the [glossary](../../GLOSSARY.md).

What exists so far (tickets 03 and 04): the project, the agent core copied from learning-agent's `apps/tau-app`, the config, safe access to `history.db`, the `GameData.db` reader, an `ADVISOR` agent with no tools of its own yet, which answers in the terminal, the importer (`just advisor import`) with its statistics, and `knowledge/`'s season sample and pair and pick queries. The FastAPI backend and the Advisor's tools come in later tickets.

## Layout

```
apps/advisor/
  pyproject.toml        uv project (hatchling): fastapi, uvicorn, sqlalchemy, httpx, tau-agent, tau-ai,
                        python-dotenv; pytest in the dev group; scripts `advisor` = agents.core.cli:main,
                        `advisor-import` = importer.cli:main
  .env.example          the settings, to copy to .env (gitignored)
  src/                  every package here is installed as it is (editable): no `src.` prefix in imports
    config.py           Config: base_url, api_key, model, port, data_dir (see Config below)
    agents/
      __init__.py       AGENTS = {"advisor": ADVISOR}
      core/             from tau-app: registry.py (AgentSpec, build_harness, MemoryState, SITUATION_KEY),
                        tooling.py (define_tool, ends_reply), provider.py (any OpenAI-compatible endpoint),
                        cli.py (the terminal runner), fake.py (a provider with set replies, for tests and --fake)
      common/tools/     ask_user.py, from tau-app
      advisor/          agent.py (ADVISOR; prepare() puts the situation in the prompt), prompt.md (one rule
                        a line), tools/ (TOOLS: only ask_user so far)
    knowledge/          the numbers, with no LLM code: cards.py (the GameData.db reader), stats.py (the
                        formulas), runs.py (imported runs loaded for them), sample.py (the season
                        sample), queries.py (the pair and pick queries)
    importer/           Scraped runs into the imported_* tables: scraped.py (one file into rows),
                        importer.py (import_builds, recompute_stats), cli.py (`just advisor import`)
    backend/            the FastAPI service (to come); db/ is here already:
      db/connection.py    open_history(): the safe open and the version check
      db/models.py        SQLAlchemy models for the advisor_* and imported_* tables
  tests/                pytest; conftest.py builds history.db with the tracker's own migrations;
                        scraped_runs.py writes run files and the record's example
```

`knowledge/` stays apart from `agents/`: it has no model in it, so its numbers can be tested on their own and checked against Bazaar DB.

### What was copied from tau-app

| File | Changed |
|---|---|
| `core/tooling.py` | Unchanged: `define_tool` checks the model's arguments (required, unknown, type, enum; `null` dropped) and raises `ValueError` for the model to read; `terminate=True` tools end the reply through `ends_reply`. |
| `core/registry.py` | tau-app's learning topics (`topics`, `TOPIC_TITLE_KEY`) left out. `SITUATION_KEY` added: the situation goes into the run's `state`, where `prepare` finds it. `MAX_TURNS` is still 12. |
| `core/provider.py` | Reads the Advisor's `Config` instead of `OPENROUTER_API_KEY`, and serves any OpenAI-compatible `base_url`, not only OpenRouter. |
| `core/cli.py` | No agent argument needed (`advisor` is the default); `--situation` and `--fake` added; `run()` takes the provider so tests can pass a fake one. |
| `common/tools/ask_user.py` | Only the docstring. |

## Config

Set in the environment or in `apps/advisor/.env` (copy `.env.example`). A real environment variable wins over the file, and an empty value counts as unset.

| Variable | Default | Meaning |
|---|---|---|
| `ADVISOR_BASE_URL` | `https://openrouter.ai/api/v1` | Any OpenAI-compatible endpoint. Changing it is all a local model needs, e.g. `http://localhost:11434/v1` (Ollama) or `http://localhost:1234/v1` (LM Studio). |
| `ADVISOR_API_KEY` | `OPENROUTER_API_KEY`, else none | The endpoint's key. OpenRouter refuses to start without one; another endpoint with no key gets no `Authorization` header. |
| `ADVISOR_MODEL` | `nvidia/nemotron-3.5-super-vl:free` | The model id, as the endpoint names it. `--model` overrides it for one `just advisor run`. |
| `ADVISOR_PORT` | `8420` | The port the backend will listen on, on `127.0.0.1` only. |
| `BAZAAR_TRACKER_DATA_DIR` | `<repo>/.bazaar-tracker` | The tracker's data folder, where `history.db` is ([data-folder.md](data-folder.md)). Set it to `%APPDATA%\BazaarTracker` for a release build that keeps its data there. |

Only the situation the app sends and the question leave the machine, and only to `ADVISOR_BASE_URL`. Run history and profile data are never sent. Nothing is uploaded to Bazaar DB.

## Commands

Recipes in `just/advisor.just`, run from `apps/advisor/`:

| Command | What it does | Plain |
|---|---|---|
| `just advisor sync` | Installs the dependencies into `apps/advisor/.venv`. | `uv sync` |
| `just advisor run -m "…"` | Asks the Advisor one question in the terminal and prints the reply, each tool call (`[tool] name(args)`), tool errors and model errors. No history, no server, no `history.db`. | `uv run advisor -m "…"` |
| `just advisor run -m "…" --situation file.json` | The same, with a situation JSON (as the app will send it) put into the system prompt. | |
| `just advisor run -m "…" --fake` | Answers with set text and calls no model: checks the plumbing without a key. | |
| `just advisor import` | Imports the Scraped runs into `history.db` and recomputes their stats (see [The importer](#the-importer)). | `uv run advisor-import` |
| `just advisor import --db PATH --builds DIR` | The same with another `history.db` or builds folder. `--min-support N` changes the fewest runs a pair needs (3); `--game-dir DIR` the folder `GameData.db` is looked for in. | |
| `just advisor test` | Runs the tests (extra arguments go to pytest). | `uv run pytest` |

Without a key and with the default `base_url`, `just advisor run` stops with "No API key for OpenRouter…". An endpoint that doesn't answer prints `[error] …` after tau_ai's retries.

## history.db rules

The Advisor shares `history.db` with the tracker ([history-db.md](history-db.md), [db-schema.html](db-schema.html)).

- **The tracker owns the schema.** The Advisor never creates the file, never migrates and never alters a table. Its models in `backend/db/models.py` describe migration 9's tables as they are.
- **The Advisor writes only `advisor_*` and the `imported_*` tables of Scraped runs** (`imported_runs`, `imported_run_cards`, `imported_picks`, `imported_combats`, `imported_pair_stats`, `imported_pick_stats`, `imported_adjacency_stats`). It never writes `run_tracker_*`.
- **`open_history(path)`** opens the existing file read-write (`mode=rw`, so a missing file is an error, not a new empty one) and sets on every connection:
  - `busy_timeout = 5000`: a write waits up to 5 s for the tracker's instead of failing;
  - `journal_mode = WAL`: readers never block the tracker's writer (the tracker uses WAL already);
  - `foreign_keys = ON`: a Conversation must belong to a Run, and the cascades work (a deleted Conversation takes its messages; a rebound or deleted Run takes its Conversations).
- **The version check.** `PRAGMA user_version` must equal `SCHEMA_VERSION` (9). Otherwise `SchemaVersionError` names both versions: an older file says to start the tracker once (`just tracker run`) to migrate it, a newer one says to update `apps/advisor`. A missing file raises `HistoryDbError`. When the tracker adds a migration, bump `SCHEMA_VERSION` and the models together; `tests/test_db.py` fails until they match.
- **Models.** `message` and `stats` are JSON stored as TEXT (SQLAlchemy's `JSON`; `stats` `None` is SQL `NULL`). `taken` is a `Boolean` stored as 0 or 1. `imported_run_cards`, `imported_picks` and `imported_combats` have no primary key, so their models map SQLite's `rowid`. `advisor_conversations.run_id` points at the tracker's `run_tracker_runs`, which has no model: SQLite enforces it.

## The card catalog

`knowledge/cards.py` reads the game's `GameData.db` the way the tracker's `CardCatalog` does ([card-catalog-and-art.md](card-catalog-and-art.md)):

- The game's data folder is `%USERPROFILE%\AppData\LocalLow\Tempo Storm\The Bazaar` (on macOS, `~/Library/Application Support/com.TempoStorm.TheBazaar`).
- The newest of `prod\cache\`, `ptr\cache\`, `cache\`, `prod\` and the folder itself wins. `ptr\cache\` is one more place than the tracker looks.
- The file is copied to `%TEMP%\bazaar_advisor_gamedata_<pid>_….db`, read with `SELECT Id, Data FROM cards` from the copy opened read-only, and the copy is deleted, even on failure. A row that isn't JSON is skipped.
- Fields are the tracker's: name (localised title, else `InternalName`, else the id), starting tier, size, type, description, art key (empty is none), heroes. `data` keeps the whole JSON for later tools.
- `get(id)` ignores case; `by_name(name)` returns every card with exactly that name, ignoring case and surrounding spaces.
- No file, or one that can't be read, gives an empty catalog, not an error.

Scraped runs' `baseId`/`cardId` are `GameData.db` ids, so cards are matched by id.

## The importer

`just advisor import` reads the run scraper's files ([scrapers.md](scrapers.md#what-it-writes)) into `history.db`. It is plain Python, with no model in it.

1. **Finds** `<data folder>/raw_data/scraper/builds/*/runs/season-*/*.json` (`season-unknown` included). The data folder is `BAZAAR_TRACKER_DATA_DIR`, else `<repo>/.bazaar-tracker`.
2. **Opens `history.db`** with `open_history`, so a file at another version than 9 is refused (an older one: start the tracker once to migrate it). A missing builds folder stops it too.
3. **Reads each file** (`importer/scraped.py`). A file that isn't JSON, isn't `format: 1`, or has no `run_id` or `hero` is skipped and listed. The same run in two folders is imported once.
4. **Replaces each run's rows**, all in one transaction: the run's old rows in the four tables are deleted, then the new ones inserted. Importing twice leaves the same rows.
5. **Checks every card id** the run mentions (Board, skills, offered and taken) against the card catalog. An id it doesn't have is still imported, and is listed with the file's title for it and how many runs mention it. With no `GameData.db`, it says the ids weren't checked.
6. **Recomputes the stats tables**: empties all three and works them out again per hero and season from every imported run (runs with no season are left out).
7. **Prints** how many runs and files, each file skipped, each hero and season's stats rows, and the unknown card ids.

What goes where:

| Table | From the file | Row |
|---|---|---|
| `imported_runs` | the top level and `summary` | `run_id`, `profile_id`, `hero`, `season`, `created_at`, `wins` (`statWins`), `losses` (`statLosses`), `victory_tier` (`victoryTier`), `url` |
| `imported_run_cards` | `summary.board` (kind `board`) and `summary.skills` (kind `skill`) | `card_id` (`baseId`), `tier` (`tierOverride`), `slot` (`slotPosition`, Board only), `size` in slots (Small 1, Medium 2, Large 3), `enchantment` (`enchantmentOverride`) |
| `imported_picks` | `timeline.events` of kind `selection` | One row per option: `day`, `hour`, `card_id`, `tier`, `taken` (the option is the `selectedCard`). The same card offered twice in one event is two rows, one taken. A `selectedCard` not among the options (some encounter steps) is added as a taken row. Rows keep the file's order, which is the order cards were taken in. Every kind of card is kept: items, skills and encounters. |
| `imported_combats` | `summary.combats` | `day` (`d`), `hour` (`h`), `outcome` (`o`: `Win`, `Loss`, or none) |

A run whose page had no timeline has no picks. On the 16 runs scraped on 8 November the import takes about a second and finds every card id in `GameData.db`.

## Statistics

`knowledge/stats.py` holds the formulas; the importer stores them per hero and season, and the queries work them out over a season sample. For runs `R` (`N` of them) and cards A and B:

| Measure | Formula | Notes |
|---|---|---|
| Support | `n(A,B)`: the runs that have both | Pairs under `MIN_SUPPORT` (3) runs are dropped from the stats tables and from `partners`. 3 keeps the record's example on today's small data; raise it as Scraped runs grow. |
| Lift | `n(A,B) × N / (n(A) × n(B))` | Above 1: built together more often than chance. |
| Rate | `(wins + α) / (wins + losses + α + β)` | A Beta prior worth `PRIOR_GAMES` (10) games, about one run, at the sample's own win rate (`α = 10 × rate`, `β = 10 × (1 − rate)`). Wins and losses are the runs' combats. A handful of runs can't top a list, and a set with no data falls back to the hero's average. |
| Win-rate gain | `rate(runs with A and B) − rate(runs with A, not B)` | "Adding B to A goes with this much more winning." None when every run with A has B. Feeds of strong runs (`?by=wins`) make it near useless: lift still works. |

The three stats tables use these with a different "together" and `n(A)`:

| Table | A and B together when | `n(A)` counts | Rows |
|---|---|---|---|
| `imported_pair_stats` | both are on the final Board or skills | runs with A on the Board or skills | both ways round: `n` and `lift` the same, `win_gain` not |
| `imported_adjacency_stats` | they sit side by side on the final Board: A's `slot + size` is B's `slot`, or the other way | runs with A on the Board | both ways round |
| `imported_pick_stats` | B was taken after A was first taken, in the same run | runs that took A | `a` then `b` |

## The season sample and the queries

`knowledge.sample(connection, hero, min_runs=10, season=None)` is the runs a hero's numbers come from: the current season's (the newest season of any imported run, so a hero with no runs this season still starts from it), plus the previous season's when there are fewer than `min_runs`. The hero ignores case. It returns the seasons used, newest first, with their run counts, the run ids, `n`, `thin` (still under `min_runs`), and `label`, e.g. `S20 (3 runs) + S19 (20 runs)`.

The queries (`knowledge/queries.py`, exported from `knowledge`) take card ids and work over that sample. Each result carries the `sample` (its seasons and counts), `n`, and the run ids behind each count:

| Query | Returns |
|---|---|
| `card_runs(connection, hero, card)` | The runs whose final Board or skills have the card (the record's "Runs with it", `4 / 15`). |
| `together(connection, hero, a, b)` | One pair whatever its support: `n`, `lift`, `win_gain`, run ids. `n` 0 when never together. |
| `partners(connection, hero, card, limit=10, min_support=3)` | The card's best partners on final Boards, most runs first, then highest lift. |
| `picks(connection, hero, card)` | Offered and taken counts from selection events (one offer seen in two events counts twice), the runs it was offered in and taken in. |
| `picked_after(connection, hero, card, limit=10, min_support=3)` | The cards most often taken after it. |

They work from the imported rows each time, with the same formulas as the stats tables, because a sample can span two seasons while the tables hold one each.

## Tests

`just advisor test`. `tests/conftest.py` builds `history.db` by running the SQL of each migration in `crates/bazaar-tracker/src/history/schema.rs`, so the tests always see the tracker's real schema.

| File | Checks |
|---|---|
| `test_db.py` | `SCHEMA_VERSION` is the number of tracker migrations; the pragmas; versions 8 and 10 refused, naming both, an older file left untouched; a missing file refused and not created; every model matches its table's columns and nullability; writing through the models, the foreign key to `run_tracker_runs`, and the cascade from a Conversation to its messages. |
| `test_cards.py` | Lookups by id and by name on a fixture `GameData.db`; the tracker's field rules; the newest file wins; the copy is deleted; nothing found is an empty catalog. |
| `test_core.py` | `define_tool`'s checks, `ends_reply`, `ask_user`, and `build_harness` with and without `prepare`. |
| `test_config.py` | Defaults, the environment, a bad port, OpenRouter needing a key, and a local `base_url` without one. |
| `test_importer.py` | Reading a run file (run fields, Board and skills, picks including a card offered twice and one taken from outside the options, combats); bad files refused and skipped; every table written; importing twice changes nothing; re-importing a run replaces its rows; an unknown card id reported and still imported; no catalog, no check; runs with no season; `advisor-import` and its refusals. |
| `test_knowledge.py` | The record's example (15 Jules runs from `scraped_runs.py`): lift 2.25 and 2.81, runs with each option, picks 15 → 7, 16 → 9, 15 → 11, the stats table and the minimum support; the win-rate gain worked out by hand; adjacency from slot and size; pick after pick; the season sample (alone, the previous season added and named, the newest season of any hero, none). |
| `test_cli.py` | The terminal runner with a fake provider: the reply, the situation in the system prompt, tool lines and tool errors, `main` with `--fake` and `--situation`, and its messages for a bad file and a missing key. |
