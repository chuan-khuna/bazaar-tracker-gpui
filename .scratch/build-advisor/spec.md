# Build Advisor: ask about the run you are playing

Status: ready-for-agent

Discussion and diagrams: [docs/records/2026-11-08.001-build-advisor-agent.html](../../docs/records/2026-11-08.001-build-advisor-agent.html). Words (Advisor, Conversation, Offer, Scraped run, Run, Live run, Snapshot, Board, Stash) are as in [GLOSSARY.md](../../GLOSSARY.md).

## Problem Statement

While playing The Bazaar, the player has to decide, many times a run, which card to take from an Offer, which items work together, which shop to visit, how to arrange the Board and which skill to take. The tracker records the run and shows the odds, but nothing helps with the decision itself. The scraper has saved Scraped runs from Bazaar DB (final Boards, skills, and what was offered and taken), and `GameData.db` describes every card, but the player can't ask a question of either.

## Solution

An **Advisor**: an agent the player chats with in a new **Advisor** tab in the Detail island. Every message carries the current situation from the app (Board, Stash, skills, stats, game state and the Offer), so "which of these should I take?" needs no typing of options. The Advisor answers from tools that compute numbers from Scraped runs and the card catalog. It ranks the options on a **recommendation card**, which shows each option's numbers, where they come from (hover to see the source runs' Boards, click to expand one), and a warning when the data is thin.

The agent is a Python service, FastAPI on `127.0.0.1:8420`, built the way `D:\code\learning-agent\apps\tau-app` builds agents (tau_agent, `AgentSpec`, `define_tool`). The gpui app is its only client. Conversations, messages and imported Scraped runs live in `history.db` beside the tracker's tables, and the tracker app owns the schema.

Before any of this, the repo moves to `apps/tracker`, `apps/scraper` and `apps/advisor`.

## User Stories

1. As a player, I want to ask the Advisor which card to take from the current Offer without typing the cards, so that I can decide quickly mid-run.
2. As a player, I want the Advisor to rank the Offer on a card with each option's art, size, tier and whether it fits on my Board, so that I can read the answer at a glance.
3. As a player, I want each option's numbers shown as bars with their sample size, so that I can compare options and see how much evidence there is.
4. As a player, I want to hover a number and see the Scraped runs behind it with their Boards, so that I can judge the evidence myself.
5. As a player, I want to expand one of those runs to see its full Board, skills, result and Bazaar DB link, so that I can learn from a concrete build.
6. As a player, I want the card to say which runs and seasons the numbers come from, and to warn me when there are fewer than 10 runs, so that I don't over-trust thin data.
7. As a player, I want an option with no data to be marked as such, with a reason from the card text, so that the Advisor never makes up numbers.
8. As a player, I want to ask what goes with an item, which shop to visit, how to arrange my Board and which skill to take, so that the Advisor helps with every kind of decision.
9. As a player, I want a chat icon on each Run in the sidebar that opens that Run's newest Conversation, so that I can ask about any Run.
10. As a player, I want a chat icon on the Main island that opens the Live run's Conversation, so that I can ask while playing in one click.
11. As a player, I want that icon greyed out when no Run is live, so that I know there is nothing to ask about yet.
12. As a player, I want the Detail island to switch between Run and Advisor tabs, and to stay on the open tab when I select another Run, so that I can move between Runs without losing my place.
13. As a player, I want a Run to have several Conversations, listed newest first, each titled with its first question and the Day and Hour, so that I can start fresh and still find old answers.
14. As a player, I want to delete a Conversation after confirming, so that the list stays tidy.
15. As a player, I want a Run's Conversations to go when I delete the Run, and to stay with it when it gets its real id, so that Conversations never point at nothing.
16. As a player, I want to see that the Advisor is thinking, with a timer, and to open what it thought afterwards, so that I know it's working and can check its reasoning.
17. As a player, I want each tool the Advisor calls shown as one line that opens to its arguments and result, so that I can see where an answer came from.
18. As a player, I want the answer to stream in with bold, lists and inline code, so that I can start reading at once.
19. As a player, I want 2–3 suggested next questions as buttons under a reply, so that the next step is one click.
20. As a player, I want a usage line under each reply (model, time, tokens, cost, tool calls), with details on hover, so that I know what it cost.
21. As a player, I want to answer the Advisor's own questions by clicking an option, so that a clarifying question is quick.
22. As a player, I want Enter to send, Shift+Enter to start a new line, and Stop to cut a reply short while keeping what arrived, so that the chat behaves as I expect.
23. As a player, I want the Advisor tab to tell me when the Advisor isn't running and how to start it, so that I'm not left with a silent failure.
24. As a player, I want only my current situation sent to the model, never my run history or profile, so that my data stays on my machine as far as possible.
25. As a player, I want to point the Advisor at a local model by changing one setting, so that I can keep everything on my machine if I choose.
26. As a player, I want statistics from the current season, with the previous season added only when the current one is thin, so that advice follows the latest patch.
27. As a developer, I want to run the Advisor in the terminal with one question, so that I can tune the prompt and tools without the app.
28. As a developer, I want to import new Scraped runs again and again without duplicates, so that I can scrape and import in any order.
29. As a developer, I want the number-crunching to be plain Python with no model in it, tested with pytest, so that the numbers can be trusted and checked against the site.
30. As a developer, I want the three apps under `apps/`, so that the repo's layout says what is in it.

## Implementation Decisions

### Repo layout
- `apps/tracker` holds the gpui app (its `Cargo.toml`, `src/`, `macos/`). The root `Cargo.toml` becomes a virtual workspace with `members = ["apps/tracker", "crates/*"]`. `crates/`, `target/`, `docs/`, `GLOSSARY.md` and `.bazaar-tracker/` stay at the root.
- `apps/scraper` replaces `app/scraper`. `just/app.just` becomes `just/tracker.just` (`just tracker …`). `just/advisor.just` is added.
- The debug build's data folder stays `<repo>/.bazaar-tracker/`. `shell::data_dir` must go up from `CARGO_MANIFEST_DIR` (now `apps/tracker`) to the repo root.
- Files are moved with `git mv`.

### history.db
- Migration 9 (Rust, in `crates/bazaar-tracker`) adds `advisor_conversations`, `advisor_messages`, `imported_runs`, `imported_run_cards`, `imported_picks`, `imported_combats`, `imported_pair_stats`, `imported_pick_stats` and `imported_adjacency_stats`. Columns are in the record's Database section.
- `advisor_conversations.run_id` references `run_tracker_runs(id)` `ON UPDATE CASCADE ON DELETE CASCADE`. `advisor_messages.conversation_id` references `advisor_conversations(id)` `ON DELETE CASCADE`.
- The tracker app owns the schema. The Advisor never migrates. It opens the file with `journal_mode=WAL`, `foreign_keys=ON` and `busy_timeout`, checks `user_version` equals the version it was written for, and refuses otherwise with a clear message. It writes only `advisor_*` and the new `imported_*` tables.
- The card catalog is read from a copy of `GameData.db` (newest under `LocalLow\Tempo Storm\The Bazaar\{prod,ptr}\cache`), as `CardCatalog` does. Scraped `baseId`/`cardId` match `GameData.db` ids, so cards are matched by id.

### The Advisor service (`apps/advisor`)
- Python with uv. `src/agents/` (tau-app's `core/` copied: registry, tooling, provider, cli; `common/tools/ask_user.py`; `advisor/agent.py`, `prompt.md`, `tools/`), `src/knowledge/` (no LLM code), `src/importer/`, `src/backend/`.
- Config (env or a file in `apps/advisor`): `base_url` (default OpenRouter), API key, model, port (default 8420), and the data folder (`BAZAAR_TRACKER_DATA_DIR`, else `<repo>/.bazaar-tracker`).
- `just advisor serve` runs FastAPI on `127.0.0.1:8420` only. `just advisor run -m "…" [--situation file.json]` runs one question in the terminal. `just advisor import` runs the importer.
- **API.** `GET /api/runs/{run_id}/conversations`, `POST /api/runs/{run_id}/conversations`, `DELETE /api/conversations/{id}`, `GET /api/conversations/{id}/messages`, and `POST /api/conversations/{id}/messages` with `{text, situation}`, which answers with an SSE stream. Each Conversation runs at most one reply at a time, and closing the stream cancels the reply. The events are tau-app's: `message_start`, `message_end`, `delta` (`kind`: text, thinking, toolcall), `stats`, `error`, `done`.
- **Situation.** JSON sent by the app: hero, day, hour, gold, level, health, game state, Board (cards with id, name, tier, size, slot, enchantment), Stash, skills, Offer (cards and their kind: merchant item, loot, encounter choice), and the opponent when in combat. `prepare()` puts it in the system prompt. It isn't stored, apart from the Day and Hour on a new Conversation.
- **Tools.** `find_card`, `rate_options`, `synergy`, `suggest`, `follow_ups`, `ask_user`, then `shop_plan`, `arrangement`, `similar_runs`. Each returns `n`, the seasons used, and the Scraped run ids behind each count. `suggest(title, ranked[{card, reason}])` doesn't end the reply. The backend adds each card's numbers and source runs to its result, so the app draws from the tool result alone. `follow_ups(questions[2..3])` doesn't end the reply either. `ask_user` ends it, as in tau-app.
- **Season sample.** The current season for the hero. When it has fewer than 10 runs, the previous season is added. Results name the seasons and the run count for each.
- **Importer.** Reads `.bazaar-tracker/raw_data/scraper/builds/*/runs/season-NN/*.json`, and replaces a run's rows on re-import (keyed by `run_id`). It then recomputes the stats tables (support, lift, win-rate gain with a Beta prior; adjacency from slot and size; pick-after-pick) per hero and season.

### The Advisor tab (`apps/tracker`)
- The Detail island gets tabs **Run** and **Advisor**. The open tab survives a change of selected Run. It isn't saved in `settings.toml`.
- Chat icons: on a sidebar Run row (on hover, and on the selected row), and on the Main island beside the pop-out button (Live run only, greyed out otherwise). Each selects the Run, opens the Detail island on Advisor, and shows the Run's newest Conversation, or creates one when the first message is sent.
- An HTTP client on a background thread (`ureq`) talks to the API. SSE lines are parsed and sent to the window through an `async_channel`, as the tracker's notices are. The parsing and the reply model are plain data with unit tests, like `src/tracking/`.
- The situation comes from `LiveState`'s `LiveView` for the Live run, and from the Run's last Snapshot for a finished Run.
- Backend not reachable: the tab says "The Advisor isn't running. Start it with `just advisor serve`." and tries again on the next send or tab open.
- A reply is drawn in this order: thinking status (`Thinking… Ns`, then a collapsed `Thought for Ns`), tool lines (spinner, ✓, ✕; collapsed, opening to arguments and result), streamed text with light markdown (bold, italics, lists, inline code; tables as plain text), the recommendation card, `follow_ups` buttons, and the usage line (hover for input, output, cache, cost and time).
- `ask_user` renders as a question with option buttons (radio or checkbox), plus a text box when `allowFreeText`. The answer is sent as an ordinary message in tau-app's format. After answering, the card is locked and rebuilt from the next user message.
- **Recommendation card.** Rows best first: art (`Names::art`), name, size, tier, fits or not, the reason, and bars with `n`. Hovering a count shows a popup of its source runs with mini Boards drawn by the existing board renderer. Clicking expands a run inside the card (full Board, skills, result, Bazaar DB link). A source line names the runs and seasons, with ⚠ under 10. With no data, the row shows "no data · from card text".
- Input: Enter sends, Shift+Enter starts a new line, and Send becomes Stop while a reply streams. Stop closes the stream, and the partial reply is kept and marked "Stopped".
- The Conversation list sits above the messages: titles from the first question's first words plus `D5 H2`, newest first, `+` for new, and ✕ on hover with a confirmation.

## Testing Decisions

- Test behaviour through public seams, not internals. Prior art: the `*_tests.rs` files next to each module in `src/tracking/` and `crates/bazaar-tracker/src/history/`, and the pure models with no gpui.
- **Migration 9:** a version-8 file migrates to 9 with a backup and every row kept. A rebound Run carries its Conversations, a deleted Run removes them, and a deleted Conversation removes its messages.
- **knowledge/ and importer (pytest):** importing twice gives the same rows. Lift, support and win-rate gain match hand-computed values on a small fixture (the record's example numbers can be a fixture). The season fallback triggers under 10 runs and names both seasons. Card ids resolve against a small `GameData.db` fixture.
- **Tools (pytest):** argument checks raise `ValueError`. `suggest` adds numbers and source runs. Results carry `n` and seasons.
- **Backend (pytest with FastAPI's test client and a fake provider):** a message streams events in order, is saved, and a stats row is written. Cancelling stops the run. A wrong `user_version` refuses to start.
- **App (Rust unit tests):** the SSE line parser (partial lines, multi-line data, unknown events). The reply model built from events (thinking timing, tool states, text, card, follow-ups, stats, stopped and failed). The situation built from a `LiveView` and from a Snapshot. Conversation titles. Which icon opens which Run and Conversation.
- By-hand checks against the game are listed in each ticket's Comments.

## Out of Scope

- Starting the backend from the app.
- The Advisor in the pop-out.
- Conversations without a Run (a General entry).
- Asking about an earlier moment of a Run (sending a selected Event's Snapshot).
- Postgres, Next.js, and any web UI.
- Scraping more data. It's needed for good answers but doesn't block any ticket.

## Further Notes

- Privacy: only the situation (card names and ids, tiers, day, gold) and the question leave the machine. This differs from the Bazaar DB upload, which stays off.
- Documents to keep in step (CLAUDE.md): `architecture.md`, `modules.md`, `gpui-app.md`, `history-db.md`, `db-schema.html`, `data-folder.md`, `scrapers.md`, `scraping-manual.html`, `odds-scraper.md`, `README.html`, the root `README.md`, and `INDEX.md`, plus a new `advisor.md` for the service. Each ticket names the ones it touches.
