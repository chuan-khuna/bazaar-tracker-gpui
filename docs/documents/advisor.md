# Advisor

The Build Advisor's service in `apps/advisor/`: a Python agent the player will ask about the Run they are playing, from the tracker's Advisor tab. It answers from Scraped runs and the card catalog, through tools that compute the numbers; the model never makes a number up. The plan and its reasons are in the [2026-11-08.001 record](../records/2026-11-08.001-build-advisor-agent.html) and `.scratch/build-advisor/spec.md`. Terms such as Run, Offer, Board and Scraped run are as in the [glossary](../../GLOSSARY.md).

What exists so far (tickets 03 to 06 and 10): the project, the agent core copied from learning-agent's `apps/tau-app`, the config, safe access to `history.db`, the `GameData.db` reader, the importer (`just advisor import`) with its statistics, `knowledge/`'s season sample and pair and pick queries, and the `ADVISOR` agent in the terminal: the situation checked and rendered into its prompt, the prompt's rules, and the tools `find_card`, `rate_options`, `synergy`, `suggest`, `shop_plan`, `arrangement`, `similar_runs`, `follow_ups` and `ask_user`; and the FastAPI backend (`just advisor serve`) with Conversations by Run, their messages and one streamed reply per message. The tracker's Advisor tab (tickets 07 to 09) lists a Run's Conversations, sends each message with the situation, and draws each reply in full, `suggest`'s result as the recommendation card with its source runs ([gpui-app.md](gpui-app.md#advisor-tab)); the other tools' results are drawn as tool lines.

## Layout

```
apps/advisor/
  pyproject.toml        uv project (hatchling): fastapi, uvicorn, sqlalchemy, httpx, pydantic, tau-agent,
                        tau-ai, python-dotenv; pytest in the dev group; scripts `advisor` = agents.core.cli:main,
                        `advisor-import` = importer.cli:main, `advisor-serve` = backend.main:main
  .env.example          the settings, to copy to .env (gitignored)
  src/                  every package here is installed as it is (editable): no `src.` prefix in imports
    config.py           Config: base_url, api_key, model, port, data_dir (see Config below)
    agents/
      __init__.py       AGENTS = {"advisor": ADVISOR}
      core/             from tau-app: registry.py (AgentSpec, build_harness, MemoryState, SITUATION_KEY),
                        tooling.py (define_tool, ends_reply), provider.py (any OpenAI-compatible endpoint),
                        cli.py (the terminal runner), fake.py (a provider with set replies, for tests and --fake)
      common/tools/     ask_user.py, from tau-app
      advisor/          agent.py (ADVISOR; prepare() checks the situation, renders it into the prompt and
                        builds the tools around it), prompt.md (one rule a line), situation.py (the
                        schema, checking, rendering), sources.py (history.db and the card catalog, opened
                        on first use), tools/ (tools_for; context.py with name resolution, find_card.py,
                        rate_options.py, synergy.py, suggest.py, shop_plan.py, arrangement.py,
                        similar_runs.py, follow_ups.py)
    knowledge/          the numbers, with no LLM code: cards.py (the GameData.db reader), stats.py (the
                        formulas), runs.py (imported runs loaded for them), sample.py (the season
                        sample), queries.py (the pair and pick queries), boards.py (a day's picks in
                        strong runs, positions and adjacency, similar runs), odds.py (the saved
                        shop and item odds)
    importer/           Scraped runs into the imported_* tables: scraped.py (one file into rows),
                        importer.py (import_builds, recompute_stats), cli.py (`just advisor import`)
    backend/            the FastAPI service (see The backend below)
      main.py             create_app(), main() for `just advisor serve` (uvicorn on 127.0.0.1)
      service.py          Service: history.db opened on first use, the Store and Runs on it, health's status
      deps.py, schemas.py FastAPI dependencies; request bodies and the situation check
      routers/            health.py, conversations.py, messages.py (the SSE reply)
      agent/              from tau-app's lib/agent: runner.py (Runs), stream.py (the events), stats.py
      db/connection.py    open_history(): the safe open and the version check
      db/models.py        SQLAlchemy models for the advisor_* and imported_* tables
      db/store.py         Store: a Run's Conversations and their messages (from tau-app's db/history.py)
  tests/                pytest; conftest.py builds history.db with the tracker's own migrations;
                        scraped_runs.py writes run files and the record's example, with a small
                        catalog and a situation for it; conftest.py's `sources` fixture is the tools'
                        sources on it
```

`knowledge/` stays apart from `agents/`: it has no model in it, so its numbers can be tested on their own and checked against Bazaar DB.

### What was copied from tau-app

| File | Changed |
|---|---|
| `core/tooling.py` | `define_tool` checks the model's arguments (required, unknown, type, enum; `null` dropped) and raises `ValueError` for the model to read; `terminate=True` tools end the reply through `ends_reply`. Added: `model_view`, a function of the result that gives what the model reads, while the whole result still goes out as `details` (`suggest` uses it to keep the source runs from the model). |
| `core/registry.py` | tau-app's learning topics (`topics`, `TOPIC_TITLE_KEY`) left out. `SITUATION_KEY` added: the situation goes into the run's `state`, where `prepare` finds it. `MAX_TURNS` is still 12. |
| `core/provider.py` | Reads the Advisor's `Config` (whose key is still `OPENROUTER_API_KEY`) instead of the environment, and serves any OpenAI-compatible `base_url`, not only OpenRouter. |
| `core/cli.py` | No agent argument needed (`advisor` is the default); `--situation` (checked against the schema) and `--fake` added; `run()` takes the provider and the tools' `Sources` so tests can pass fake ones. |
| `common/tools/ask_user.py` | Only the docstring. |

## Config

Set in the environment or in `apps/advisor/.env` (copy `.env.example`). A real environment variable wins over the file, and an empty value counts as unset.

| Variable | Default | Meaning |
|---|---|---|
| `ADVISOR_BASE_URL` | `https://openrouter.ai/api/v1` | Any OpenAI-compatible endpoint. Changing it is all a local model needs, e.g. `http://localhost:11434/v1` (Ollama) or `http://localhost:1234/v1` (LM Studio). |
| `OPENROUTER_API_KEY` | none | The endpoint's key, sent as `Authorization: Bearer <key>`; the only name read for it (`ADVISOR_API_KEY` was dropped, see below). Spaces, matching quotes and invisible characters (a byte-order mark, zero-width spaces) around it are removed. OpenRouter refuses to start without one; another endpoint with no key gets no `Authorization` header. |
| `ADVISOR_MODEL` | `nvidia/nemotron-3.5-super-vl:free` | The model id, as the endpoint names it. `--model` overrides it for one `just advisor run`. |
| `ADVISOR_PORT` | `8420` | The port `just advisor serve` listens on, on `127.0.0.1` only. |
| `BAZAAR_TRACKER_DATA_DIR` | `<repo>/.bazaar-tracker` | The tracker's data folder, where `history.db` is ([data-folder.md](data-folder.md)). Set it to `%APPDATA%\BazaarTracker` for a release build that keeps its data there. |

**Warnings.** `config.load()` keeps notes on settings that look wrong, and `just advisor run` (on stderr) and `just advisor serve` (in its log, at start) print them. They never include the key. There are two: `ADVISOR_API_KEY` is still set (it is no longer read: move the key to `OPENROUTER_API_KEY` and remove it), and, for OpenRouter, a key that doesn't start with `sk-or-` (OpenRouter answers such a key with 401 "Missing Authentication header"). The key is read once, at start: restart `serve` after changing it.

Why `ADVISOR_API_KEY` went: it was read before `OPENROUTER_API_KEY`, so a stale or placeholder value in it won over the real key. The provider then sent that value as the Bearer token and OpenRouter answered 401 "Missing Authentication header", the message it gives for a token that isn't an OpenRouter key, while `config.load()` still reported a key set.

Only the situation the app sends and the question leave the machine, and only to `ADVISOR_BASE_URL`. Run history and profile data are never sent. Nothing is uploaded to Bazaar DB.

## Commands

Recipes in `just/advisor.just`, run from `apps/advisor/`:

| Command | What it does | Plain |
|---|---|---|
| `just advisor sync` | Installs the dependencies into `apps/advisor/.venv`. | `uv sync` |
| `just advisor run -m "…"` | Asks the Advisor one question in the terminal and prints the reply, each tool call (`[tool] name(args)`), tool errors and model errors. No Conversation history and no server; the tools read the data folder's `history.db` and the game's `GameData.db`. | `uv run advisor -m "…"` |
| `just advisor run -m "…" --situation file.json` | The same, with a situation JSON (as the app will send it, see [The situation](#the-situation)) checked and put into the system prompt. A file that isn't valid stops it, naming each bad field. | |
| `just advisor run -m "…" --fake` | Calls no model: checks the plumbing without a key. With an Offer in the situation, the fake plays the Advisor's usual turn so the tools really run: `rate_options` on the Offer, `suggest` with the Offer in its order, `follow_ups`, then set text. Without one, set text only. | |
| `just advisor serve` | Runs the backend for the tracker's Advisor tab on `127.0.0.1:8420` (see [The backend](#the-backend)). Stop it with Ctrl+C. | `uv run advisor-serve` |
| `just advisor serve --fake` | The same, answering with the fake provider instead of a model: no key needed. | |
| `just advisor import` | Imports the Scraped runs into `history.db` and recomputes their stats (see [The importer](#the-importer)). | `uv run advisor-import` |
| `just advisor import --db PATH --builds DIR` | The same with another `history.db` or builds folder. `--min-support N` changes the fewest runs a pair needs (3); `--game-dir DIR` the folder `GameData.db` is looked for in. | |
| `just advisor test` | Runs the tests (extra arguments go to pytest). | `uv run pytest` |

Without a key and with the default `base_url`, `just advisor run` stops with "No API key for OpenRouter…". An endpoint that doesn't answer prints `[error] …` after tau_ai's retries. A `history.db` that is missing or at another version than 9 doesn't stop the run: each tool that needs it fails with the reason, which the model reads.

`run` and `import` pass each argument on as it was typed, so `-m "Which of these should I take?"` stays one argument: on Windows the recipe is a PowerShell script that splats `$args` to `uv`, elsewhere it passes `"$@"`.

## The backend

`just advisor serve` runs the FastAPI service the tracker's Advisor tab talks to (`backend/main.py`), with uvicorn on `127.0.0.1` only (`HOST`, not configurable) and port `ADVISOR_PORT` (8420). The app on the same machine is its one client: there is no authentication and no CORS. `--fake` answers with the fake provider (as `run --fake`: the tools really run on the Offer), so the app can be built and tried without a key. Without `--fake`, a missing OpenRouter key stops it at once with "No API key for OpenRouter…".

The service starts whatever state `history.db` is in, so `GET /api/health` can say what is wrong. The file is opened with `open_history` (the version check) the first time a route needs it, and again on each later request until it opens: after the tracker has migrated an older file, the Advisor works without a restart. Until then every route but health answers **503** with the reason, such as "… is at schema version 8, but the Advisor is written for version 9. Start the tracker (`just tracker run`) once to migrate it." The service never migrates or creates the file.

What it was built from, in tau-app's backend: `Runs` (`backend/agent/runner.py`), the stream events (`agent/stream.py`), the reply stats (`agent/stats.py`), the Store (`db/store.py`, from `db/history.py`), `deps.py` and the routers. Changed: Conversations belong to a Run rather than an agent, the situation goes into each run's state, the provider is made per message so the fake can read the message and the situation, a stopped reply keeps its partial message (below), and the cost is the provider's (tau-app's OpenRouter price list is left out).

### The API

All under `/api`. Bodies and replies are JSON, with camelCase keys as tau_agent's messages have. An error is `{"detail": "…"}`.

| Route | Body | Answers |
|---|---|---|
| `GET /api/health` | | 200 `{ok, version, historyDb: {path, version, expected, error}}`: the Advisor's version (`pyproject.toml`), `history.db`'s `user_version` (`null` when the file is missing or can't be read) and the 9 it needs; `ok` false and `error` saying why while it can't be used. Always 200 while the service runs. |
| `GET /api/runs/{run_id}/conversations` | | 200, the Run's Conversations newest first. 404 for a Run not in `run_tracker_runs`. |
| `POST /api/runs/{run_id}/conversations` | `{text, situation?}` | 201, the new Conversation: titled by the first words of `text` (the first question), with the situation's `day` and `hour` (`null` when not sent). 404 for an unknown Run; 400 for an empty `text` or a situation that isn't valid (naming each bad field). The app then sends the same question to its messages. |
| `DELETE /api/conversations/{id}` | | 204; its messages go by the schema's cascade. 404 when unknown, 409 while it is replying. |
| `GET /api/conversations/{id}/messages` | | 200 `{running, messages: [{seq, message, stats, step, createdMs}]}`: `running` while a reply streams; `message` is tau_agent's message (`role` `user`, `assistant` or `toolResult`); `stats`, the reply's total, only on a reply's last assistant message; `step`, that message's own step stats, on every assistant message (`null` on the others, on a stopped message, and on messages saved before step stats). Both have the shape in [Stats](#stats). 404 when unknown. |
| `POST /api/conversations/{id}/messages` | `{text, situation?}` | 200, `text/event-stream`: the reply (below). 400 for an empty `text` or an invalid situation, 404 when unknown, 409 "A reply is still running." while the Conversation has a reply streaming. |

A **Conversation** is `{id, runId, title, day, hour, createdMs}`. The title is the question on one line, cut on a word to 48 characters with `…`; the tab adds `D5 H2` from `day` and `hour`. Times are Unix ms. The **situation** is the JSON in [The situation](#the-situation); it is checked on both POSTs, goes into that message's system prompt, and is never stored, apart from a new Conversation's Day and Hour.

### The reply stream

Each event is one SSE frame, its `type` as the event name and the whole event as one line of JSON:

```
event: delta
data: {"type": "delta", "kind": "text", "index": 0, "delta": "Take the"}

```

| `type` | Payload | When |
|---|---|---|
| `message_start` | `message` | A message begins: the user's, each assistant message (one per model turn, `content` still empty), each tool result. |
| `delta` | `kind` (`text`, `thinking` or `toolcall`), `index` (the content block in the assistant message), `delta` (the new text, or a piece of a tool call's JSON arguments) | While an assistant message streams. |
| `message_end` | `message` | A message is whole, and saved. An assistant message carries `content` (`text`, `thinking` and `toolCall` blocks), `usage`, `stopReason` and, on a failed model call, `errorMessage`. A tool result carries `toolCallId`, `toolName`, `content` (what the model read), `details` (the whole result: the app draws the recommendation card from `suggest`'s) and `isError`. |
| `step` | `seq` (the assistant message it was saved on), `step`: that model call's stats ([Stats](#stats)) | Right after each assistant `message_end`. |
| `stats` | `seq` (the message it was saved on), `stats`: the reply's total, `{model, elapsedMs, tokens: {input, output, cacheRead, cacheWrite, total}, costUsd, pricingSource: "provider", toolCalls}` | After the reply's last message, when it had an assistant message. |
| `error` | `error`: the message | The reply failed outside the model call (a model's own error comes as an assistant `message_end` with `errorMessage`). |
| `done` | | Always last. |

A reply in order: `message_start`/`message_end` for the question; then per model turn `message_start` (assistant), `delta`s, `message_end` (assistant), `step`, and for each tool call `message_start`/`message_end` (tool result); then `stats` and `done`. It ends when the model answers without calling a tool, after `ask_user` (which ends it, as in tau-app), at 12 model turns, or when the stream is closed.

**One reply at a time.** A Conversation with a reply streaming refuses another message (409) and deletion (409); its `GET …/messages` says `running: true`. Different Conversations can reply at once.

**Saving.** Each message is saved to `advisor_messages` as it ends, through a harness subscriber, so a closed stream loses nothing that ended; each assistant message is saved with its step, and the reply's total goes on its last assistant message beside that step. The next message's model history is every saved message but failed replies (`stopReason: "error"`).

**Stopping.** Closing the stream cancels the reply: the harness is cancelled and closed, a tool call left without a result is saved with the result "Tool call interrupted by user", and the assistant message still streaming is saved with the text and thinking that arrived and `stopReason: "aborted"` (nothing when none had arrived). A stopped reply gets no `stats`, and its cut-short message no `step` (the steps that ended keep theirs). The app shows it as "Stopped", and it is part of the next message's history.

### Stats

A **step** is one model call, which ends in one assistant message. Both a step's stats and a reply's are `{model, elapsedMs, tokens: {input, output, cacheRead, cacheWrite, total}, costUsd, pricingSource: "provider", toolCalls}` (`backend/agent/stats.py`):

| Field | A step | The reply |
|---|---|---|
| `model` | The model that answered (the provider's `response_model`, else the message's `model`, else `ADVISOR_MODEL`) | `ADVISOR_MODEL` (`fake` under `--fake`) |
| `elapsedMs` | From the call's start (the end of the message before it: the question or the last tool result) to the assistant message's end | From the question to the end of the last step, tool calls included |
| `tokens`, `costUsd` | The message's `usage`: what the provider reported (a local model reports no cost, so 0) | The sum over the steps |
| `toolCalls` | The tool calls in the message | The sum over the steps |

Saved in `advisor_messages.stats`, with no schema change: every assistant message holds `{"step": {…}}`; a reply's last assistant message holds the total's fields with `step` beside them, `{model, elapsedMs, …, "step": {…}}`. `GET …/messages` splits the column into `stats` (the total) and `step`. A row saved before step stats holds the total alone and reads as `stats` with `step: null`. The tracker draws only the total (the usage line); it passes over the `step` event and field.

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
- `get(id)` ignores case; `by_name(name)` returns every card with exactly that name, ignoring case and surrounding spaces. `search(query)`, for `find_card`: the card with that id, else exactly that name, else names containing it, else names containing every word of it (shortest names first).
- From `data`, for the tools: `text` (the description, else the tooltips one per line, `{ability.0}` placeholders left in), `tags` (`Tags` then `HiddenTags`), `attributes` (the starting tier's) and `mechanics` (the tags and every tier's attribute keys, less `CooldownMax`, `Multicast`, `BuyPrice` and `SellPrice`: what two cards share when they work on the same thing).
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
| `picks(connection, hero, card, day=None)` | Offered and taken counts from selection events (one offer seen in two events counts twice), the runs it was offered in and taken in. `day`: that day's events only. |
| `picked_after(connection, hero, card, limit=10, min_support=3)` | The cards most often taken after it. |
| `rate_options(connection, hero, cards, board, day=None)` | Each option over one sample: its `card_runs`, its `picks`, and `together` with each Board card (with that Board card's own run count, whatever the support); `no_data` when it was never on a final Board nor offered. |

`knowledge/boards.py` adds three more over the same sample:

| Query | Returns |
|---|---|
| `day_picks(connection, hero, day)` | Every card offered on that day in the sample's strong runs (`STRONG_WINS`, 10 wins or more, a victory), with how often it was offered and taken and the runs that took it, most taken first; and the strong runs' ids. `imported_picks` has every kind of card and no card type, so the caller tells encounters apart with the card catalog. |
| `arrangement(connection, hero, cards)` | Each pair of the cards side by side on a final Board (`adjacency_stats`, whatever the support), with the runs whose Board has both; and each card's `Position`: the runs with it on the Board, its first slot with a count for each, and how many Boards had nothing to its left (`leftmost`) and to its right (`rightmost`). |
| `similar_runs(connection, hero, cards, limit=5)` | The runs whose final Board shares the most of the cards, then the closest (Jaccard: shared over the cards in either Board), then the most wins; runs sharing none are left out. |

`knowledge/odds.py` reads Bazaar DB's odds as the tracker saved them: `tier_odds(connection, day)` is `imported_shop_odds`' row for the day, else the last saved day before it (`row_day`), with `samples`, the percent of each tier and `source`; `item_odds(connection, card, hero, day, tier="None")` is `imported_item_odds`' row for the card (the id lower-cased, as the tracker saves it) and hero, for the tier asked when saved, else any tier, with its `body` read as `bazaardb::item_odds::parse_json` reads it: the day's column (its own, or a last `N+` that covers it), whether it is measured, the "any shop" chance and each merchant's chance after one reroll, best first, merchants at 0 left out. A body that isn't JSON reads as no data. Nothing is found: `None`.

`knowledge/runs.py` also gives `run_summaries(connection, run_ids)` (each run's result, final Board in slot order, skills and URL, to show as a source) and `profile_count(connection, run_ids)`.

They work from the imported rows each time, with the same formulas as the stats tables, because a sample can span two seasons while the tables hold one each.

## The situation

The JSON the app sends with each message (and `--situation` reads), checked by `agents/advisor/situation.py` (pydantic). Every field may be left out, since the app sends what the game has shown so far; a field that is there must have the right type, and an unknown field is refused so a misspelt key is caught. Text is stripped of surrounding spaces.

| Field | Type | Meaning |
|---|---|---|
| `hero` | string | The hero, as the game names it. The tools' default hero. |
| `day`, `hour` | integer (day ≥ 1, hour ≥ 0) | The time in the Run. |
| `gold`, `level`, `health`, `health_max` | integer | The player's numbers. |
| `game_state` | string | The game's state in words: `Shop`, `Encounter choice`, `Combat`, `Loot`, … |
| `board` | list of cards | The Board. Left out: not known; `[]`: empty. |
| `stash`, `skills` | list of cards | The Stash and the skills, the same way. |
| `offer` | list of Offer cards | What is on offer now. |
| `opponent` | object | In a combat: `name`, `hero`, `health`, `health_max` (all optional), `board` and `skills` (lists of cards). |

A **card** is `{id, name, tier, size, slot, enchantment}`: `name` is required and not empty; `id` is its `GameData.db` id (the Snapshot's `template_id`), which the tools match first; `size` is `Small`, `Medium` or `Large` (any case); `slot` is the first Board slot it takes, 0–9. An **Offer card** adds `kind`, required: `merchant_item`, `loot`, `encounter_choice` or `skill`.

```json
{
  "hero": "Jules", "day": 5, "hour": 2, "gold": 12, "level": 4, "health": 900, "health_max": 1200,
  "game_state": "Shop",
  "board": [
    {"id": "b0f345cb-a769-487d-9fd4-fa5fe130f8b9", "name": "Giant Lollipop", "tier": "Silver", "size": "Large", "slot": 1},
    {"id": "eaad7107-19ac-4f35-b1fb-9359ca33968d", "name": "Pizza Cutter", "tier": "Silver", "size": "Small", "slot": 4}
  ],
  "stash": [], "skills": [],
  "offer": [
    {"name": "Decanter", "tier": "Gold", "size": "Small", "kind": "merchant_item"},
    {"name": "Laurel's Bouncy Ball", "tier": "Gold", "size": "Small", "kind": "merchant_item"},
    {"name": "Cheese Wheel", "tier": "Silver", "size": "Medium", "kind": "merchant_item"}
  ]
}
```

A bad situation is a `SituationError` (a `ValueError`) naming each bad field, such as `offer.0.kind: Field required`; `--situation` stops with it. The situation goes into the run's state under `SITUATION_KEY`, and `prepare()` renders it as the prompt's `## Situation` section: a line with the hero, day, hour and game state, a line with gold, level and health, then each list one card a line as `name (kind, size, tier, enchantment) [id …]`, the Board with each card's slot and `(4 of 10 slots used, 6 free)`, and "not sent" or "empty" for a list left out or empty. With no situation the section says so; with an invalid one (which only a client that skipped the check can send) it gives the error and tells the model to ask.

## The tools

`agents/advisor/tools/`. `tools_for(situation, sources)` builds them for each run, because seven of them read the situation and the `Sources`: `history.db` and the card catalog, opened the first time a tool needs them (`agents/advisor/sources.py`). The CLI passes `Sources.from_config`; without one in the run's state, `prepare` uses `default_sources()`, also from the config. The numbers all come from `knowledge/`: a tool checks its arguments, names cards to ids, calls `knowledge/`, and returns JSON.

**Card names.** The model names cards as the situation or `find_card` shows them. A name is matched, ignoring case, against the situation's cards (by id, then by name), then the card catalog by id, then by name, preferring a card the hero can get (`knowledge/cards.py`). No match is a `ValueError` that says to use `find_card`.

**Every tool that returns numbers** (`find_card`, `rate_options`, `synergy`, `suggest`, `shop_plan`, `arrangement`, `similar_runs`) returns `n` (the runs looked at), `seasons` (`[{season, runs}]`, newest first), `sample` (`S20 (15 runs)`) and `thin` (under 10 runs), from the hero's season sample, and the run ids behind each count. The hero is the `hero` argument, else the situation's; neither is a `ValueError`. **Every tool raises `ValueError` on bad arguments** (wrong types and unknown keys through `define_tool`, then each tool's own checks below), which the model reads as an error tool result. A `history.db` that can't be used is a `ValueError` too.

| Tool | Arguments | Returns |
|---|---|---|
| `find_card` | `query`, `hero?`, `type?` | Up to 8 `matches` from `CardCatalog.search`, kept to the hero's and neutral cards with `hero`, and to a card type with `type`: `card`, `id`, `type`, `size`, `tier` (starting), `heroes`, `text`, `tags`, `attributes`, and `runs_with` (`n` of `of` runs) for `hero` or the situation's hero. With no hero, or no usable `history.db` (then a `note` says why), no counts and `n` 0. Errors: an empty query; an empty catalog. |
| `rate_options` | `options?` (1–10 names), `hero?`, `day?` (1–30) | `options` default to the situation's Offer (an error with neither). For each: `card`, `id`, `kind`, `size`, `tier`, `fits` (an item's slots against the Board's free slots; `null` for a skill or encounter, or when the Board or a size isn't known), `no_data`, `runs_with` (`n`, `of`, `run_ids`), `picks` (`offered`, `taken`, `take_rate`, `offered_run_ids`, `taken_run_ids`; with `day`, that day's offers only), and `with_board`: for each situation Board card, `n` runs with both out of `of` runs with the Board card, `lift`, `win_gain`, `run_ids`. |
| `synergy` | `cards` (1–5 names), `hero?`, `limit?` (1–20, default 8) | For each card: `n` runs with it, `run_ids`, and its `partners` (`partners` in `knowledge/`, support 3 or more): `card`, `id`, `n`, `lift`, `win_gain`, `run_ids`, `shared_mechanics` (the two cards' `mechanics` in common) and `on_your_board`. Also `min_support`. |
| `suggest` | `title`, `ranked` (1–10 of `{card, reason}`), `hero?` | The recommendation card. `ranked` rows in the model's order with `rank` and `reason`, each with `rate_options`' numbers; `source`, the source line (`15 Jules runs · S20 · 1 profile`, `· thin ⚠` under 10 runs, `no Scraped Vanessa runs · thin ⚠` with none); `profiles`; and `runs`, every run behind a row's numbers by id, with `hero`, `season`, `wins`, `losses`, `victory_tier`, `url`, `board` (`id`, `name`, `tier`, `slot`, `size` in slots, `enchantment`) and `skills`, for the card's hover and expand. The model reads it without `runs` and the run ids (`model_view`), and is told the card is drawn. Errors: an empty title or reason, a card twice, an unknown card. It doesn't end the reply. |
| `shop_plan` | `wanted?` (1–10 names), `hero?`, `day?` (1–30) | `day` defaults to the situation's (an error with neither). `n` and the seasons from the season sample. `tier_odds`: `tier_odds` for the day (`day` is the row used, `samples`, `bronze_pct` … `diamond_pct`, `source`), or `null` with a `tier_odds_note`. `wanted`: for each card, `owned_tier` (its tier on the situation's Board or Stash, else `null`), and Bazaar DB's odds for that tier (else any saved tier, `odds_tier`): `column`, `measured`, `any_shop_pct`, the best 5 `merchants` (`merchant`, `chance_pct`, `seen_pct`, `always_tier`), `source`; or `no_odds` with a `note`. `encounters`: `strong_runs`, `rule` (`runs with 10+ wins`) and up to 8 `picks` from `day_picks` whose catalog type ends in `Encounter` (merchants, events, fights; not `EncounterStep`): `card`, `id`, `type`, `taken`, `offered`, `run_ids`; with an empty catalog no picks and a `note`. Errors: no day, an unknown card. |
| `arrangement` | `board?` (2–10 names), `hero?` | `board` defaults to the situation's Board; either must have 2 different known cards. `free_slots`; `pairs` from `arrangement` (`cards`, `ids`, `n` runs side by side, `of` runs with both on the Board, `lift`, `run_ids`); `positions` for each card (`card`, `id`, `size`, `slot_now` on the situation's Board, `n`, `slots` as first slot to runs, `leftmost`, `rightmost`, `run_ids`); and `position_text`, each card whose text mentions adjacent, left, right, leftmost, rightmost or neighbours. The model proposes the order. |
| `similar_runs` | `cards?` (1–10 names), `hero?`, `limit?` (1–10, default 5) | `cards` default to the situation's Board (an error when it has no known card). `cards` as named, and `runs` from `similar_runs`, each with `shared`, `overlap` (of `of` cards), `jaccard`, and the run as `suggest` gives it: `run_id`, `hero`, `season`, `wins`, `losses`, `victory_tier`, `url`, `board`, `skills`. The model reads all of it. |
| `follow_ups` | `questions` (2–3) | `{questions}`, stripped: the buttons under the reply. Errors: fewer than 2 or more than 3 different questions, an empty one, one over 120 characters. It returns no numbers, so no `n`. It doesn't end the reply. |
| `ask_user` | `question`, `options` (2–6 of `{id, label}`), `multiple?`, `allowFreeText?` | From tau-app: `{status: "waiting_for_user"}`, and it ends the reply. No numbers, no `n`. |

On the 16 runs scraped on 8 November, `rate_options` on the record's example Offer gives Laurel's Bouncy Ball 4 / 15 runs, offered 15 and taken 7, with Giant Lollipop 3 of 5 (lift 2.25) and Pizza Cutter 3 of 4 (lift 2.81), as in the record. Decanter and Cheese Wheel come out offered 17 times (the record says 16 and 15) and Cheese Wheel never with Giant Lollipop (the record says once). The run files agree with the tool: each card is among the options in 17 selection events, and no final Board has both. The record's table is left as written; `tests/scraped_runs.py` builds the record's numbers as a fixture.

## The prompt rules

`agents/advisor/prompt.md`, one rule a line, followed by the `## Situation` section:

- Read the situation first; never ask for what it shows.
- Use only numbers from tools, each with its `n` ("on 3 of 5 final Boards with Giant Lollipop", "taken 7 of 15 times offered"); never invent or estimate one.
- Under 10 runs, say the data is thin and how many runs it comes from.
- An option with no data: say so, and reason from its card text (`find_card`).
- Name cards as the situation or `find_card` names them.
- To rank an Offer or skills: `rate_options`, `synergy` when the Board matters, then `suggest` with the ranking and a one-line reason each.
- `suggest` draws the card: don't repeat it as a list, but still write the answer.
- A shop, an encounter or a day's plan: `shop_plan` with the wanted cards; odds as its percent, said to be Bazaar DB's.
- Arranging the Board: `arrangement`, then the model proposes the order.
- What builds like the player's ended as: `similar_runs`, with each run's result and link.
- Short answers: the decision, the reason, how sure.
- End every reply with `follow_ups` (2–3 questions).
- `ask_user` only when the question is unclear.

## Tests

`just advisor test`. `tests/conftest.py` builds `history.db` by running the SQL of each migration in `crates/bazaar-tracker/src/history/schema.rs`, so the tests always see the tracker's real schema.

| File | Checks |
|---|---|
| `test_db.py` | `SCHEMA_VERSION` is the number of tracker migrations; the pragmas; versions 8 and 10 refused, naming both, an older file left untouched; a missing file refused and not created; every model matches its table's columns and nullability; writing through the models, the foreign key to `run_tracker_runs`, and the cascade from a Conversation to its messages. |
| `test_cards.py` | Lookups by id and by name on a fixture `GameData.db`; the tracker's field rules; the newest file wins; the copy is deleted; nothing found is an empty catalog. |
| `test_core.py` | `define_tool`'s checks, `ends_reply`, `ask_user`, and `build_harness` with and without `prepare`. |
| `test_config.py` | Defaults, the environment, a bad port, OpenRouter needing a key, and a local `base_url` without one; the key from `OPENROUTER_API_KEY` only (`ADVISOR_API_KEY` ignored with a note, and a placeholder in it no longer winning); spaces, quotes and invisible characters removed from the key; the note on a key that isn't an OpenRouter key, without its value; the key never in the config's `repr`; `load()` reading a `.env` under the real environment; and the `Authorization: Bearer` header the provider sends, caught by a mock transport (none for a local model without a key). Every key in the tests is made up. |
| `test_importer.py` | Reading a run file (run fields, Board and skills, picks including a card offered twice and one taken from outside the options, combats); bad files refused and skipped; every table written; importing twice changes nothing; re-importing a run replaces its rows; an unknown card id reported and still imported; no catalog, no check; runs with no season; `advisor-import` and its refusals. |
| `test_knowledge.py` | The record's example (15 Jules runs from `scraped_runs.py`): lift 2.25 and 2.81, runs with each option, picks 15 → 7, 16 → 9, 15 → 11, the stats table and the minimum support; the win-rate gain worked out by hand; adjacency from slot and size; pick after pick; the season sample (alone, the previous season added and named, the newest season of any hero, none). |
| `test_situation.py` | A full situation; every field optional; sizes in any case; bad situations refused naming the field; the Offer kinds; the rendering; the system prompt with none, one and an invalid one; the prompt's rules. |
| `test_tools.py` | Each tool on the record's example (`scraped_runs.py`, a small catalog, and a situation with Giant Lollipop and Pizza Cutter on the Board and the record's Offer): `rate_options` defaulting to the Offer (4 / 15, 15 → 7, lift 2.25 and 2.81), by name, on a day, on a full Board and with no data; `synergy`'s partners and shared mechanics; `find_card` with and without a hero, its filters, an empty catalog and no `history.db`; `suggest`'s numbers, source runs, source line and what the model reads, thin and with no data; `suggest` and `follow_ups` not ending the reply; and every tool's `ValueError`s. |
| `test_plan_tools.py` | `knowledge/odds.py` reading an item's odds (a day's column, a `6+` column not measured, a merchant at 0 left out, a bad body), the tier odds for a day and the last day before it, and `day_picks` leaving out a run with 3 wins; `shop_plan` on saved shop and item odds and three runs that chose encounters on day 5 (the owned tier's odds, no odds for a card, an `EncounterStep` left out), on another day and with no odds or catalog; `arrangement` on the record's Boards (Giant Lollipop then Pizza Cutter side by side in 2 runs, lift 1.5, their slots, leftmost counts and position text) and by name; `similar_runs` ranked by overlap then Jaccard, with each run's result, Board and link; and each tool's `ValueError`s. |
| `test_backend.py` | The service with fake providers: health with the versions; a version-8 file refused (503 naming both versions, left at 8), then used once it is at 9 without a restart; a missing file reported, not created; a new Conversation's Run, title, Day and Hour, newest first; 404 for an unknown Run or Conversation; 400 for an empty question or an invalid situation; deleting with its messages; a reply's events in order, saved, with a stats row and its `step`; the history on the next message; `thinking`, `toolcall` and `text` deltas and the usage in the stats; a two-step reply (a tool call, then text) with each step's model, tokens, cost, tool calls and time, the total the steps' sum, both reloaded, and the `stats` column's layout; a row saved before step stats read as the total; a fake Advisor turn running `rate_options`, `suggest` and `follow_ups` with every message saved; `ask_user` ending the reply; a failing provider; and, in uvicorn on a free port, 409 for a second message and a delete while a reply streams, then closing the stream cancelling it and keeping the partial reply as `aborted`. |
| `test_cli.py` | The terminal runner with a fake provider: the reply, the situation in the system prompt, tool lines and tool errors, a run that calls `rate_options`, `suggest` and `follow_ups` and goes on to the answer, `--fake` running the tools on the Offer (also through `main` on a data folder), and its messages for a bad file, an invalid situation and a missing key. |
