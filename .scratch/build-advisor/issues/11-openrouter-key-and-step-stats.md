# 11: Load the key from OPENROUTER_API_KEY, and save stats for every step

**What to build:** Two changes in `apps/advisor`, asked for by the user on 2026-11-08:

1. The API key comes from `OPENROUTER_API_KEY` (environment or `apps/advisor/.env`). Drop `ADVISOR_API_KEY` so there is one name for the key.
2. A Conversation keeps stats for each step of a reply, not only one total per reply. A step is one model call (one assistant message). Each step saves its token usage (input, output, cache read, cache write), cost, time taken and the model used. The reply total stays as it is.

**Blocked by:** 10

**Status:** resolved

- [x] Bug from the user's screenshot: in the gpui app a reply failed with "OpenRouter request failed with status 401 for model nvidia/nemotron-3.5-super-vl:free: Missing Authentication header", while `config.load()` reports a key is set. Find out why `just advisor serve` sent no (or an empty) Authorization header (e.g. `ADVISOR_API_KEY` in `.env` holding a blank or placeholder that wins over `OPENROUTER_API_KEY`, whitespace or quotes not stripped, the server process not loading `.env`, the provider's `omit_authorization_header`), fix it, and add a test. Don't print or log key values; the user's `apps/advisor/.env` is theirs, so don't edit it — say what to change if it needs changing.
- [x] `config.py` reads the key from `OPENROUTER_API_KEY` only. `.env.example`, the "No API key" message and the docs name it.
- [x] Each assistant message saved in `advisor_messages` carries its step's stats: model, input, output, cache read and write tokens, cost, and time (ms) from the call's start to its end. The stats of the reply's last message still hold the reply total, alongside its own step.
- [x] The SSE stream sends each step's stats as the step ends (a documented event or a field on `message_end`), and the final `stats` event keeps its current shape.
- [x] `GET /api/conversations/{id}/messages` returns each message's step stats, so a reloaded Conversation has them.
- [x] No schema change in `history.db`: the stats live in the existing `stats` JSON column.
- [x] pytest: a fake two-step reply (tool call, then text) saves two steps with their tokens, cost, time and model; the total equals the sum; reload returns them; the key comes from `OPENROUTER_API_KEY`.
- [x] Documents: `advisor.md` (config, the stats and their payloads).

## Comments

### 2026-11-08 — done (agent)

**The 401's cause.** The key never reached OpenRouter as an OpenRouter key, though one was "set":

- `config.from_env` read `ADVISOR_API_KEY` first and `OPENROUTER_API_KEY` only when the first was empty. The user's `apps/advisor/.env` has both names (checked by name only: `grep -oE '^[A-Z_]+=' .env`), and neither is set in the real environment (Process, User or Machine scope), so the key came from `.env`. A stale or placeholder value in `ADVISOR_API_KEY` wins over the real key in `OPENROUTER_API_KEY`, and `config.load()` still reports a key set.
- The provider (`OpenRouter` branch, `omit_authorization_header` false) sends whatever key it was given as `Authorization: Bearer <key>` (tau_ai `openai_compatible.py` `_stream`). The error text proves this branch ran with a non-empty key: the message names "OpenRouter" (only the OpenRouter branch sets that provider name) and that branch refuses to start without a key.
- OpenRouter answers 401 "Missing Authentication header" not only when the header is missing but when the Bearer token isn't an OpenRouter key (reports of the same error with an Anthropic key, or a key missing its `sk-or-` prefix).
- Ruled out: `.env` not loaded under `serve` (`load_dotenv(APP_DIR / ".env")` uses an absolute path, the same function as `config.load()`); blank or whitespace values (`get` strips and treats empty as unset, so a blank `ADVISOR_API_KEY=` fell through correctly); quotes in `.env` (python-dotenv drops them); the Responses endpoint (only `gpt-5.4/5.5`/`codex` models go there).
- I did not read the key values, so I can't say for certain which value was wrong; the startup warnings below will now name the problem.

**The fix.** `OPENROUTER_API_KEY` is the only name read (`ADVISOR_API_KEY` dropped). The key is cleaned of spaces, matching quotes (a real environment variable keeps them) and invisible characters (BOM, zero-width). `Config.notes` warns, without the value, when `ADVISOR_API_KEY` is still set, and when an OpenRouter key doesn't start with `sk-or-`; `just advisor run` prints them on stderr and `just advisor serve` logs them at start. `Config.__repr__` hides the key. Tests catch the real `Authorization` header with an httpx `MockTransport`.

**What the user should do in `apps/advisor/.env`:** put the OpenRouter key (from https://openrouter.ai/settings/keys, starts with `sk-or-`) in `OPENROUTER_API_KEY`, delete the `ADVISOR_API_KEY=` line, and restart `just advisor serve` (the key is read once at start). If `serve` then warns that the key doesn't look like an OpenRouter key, copy it again.

**Step stats.** `ReplyStats.add` returns each step's stats (same shape as the total: model, elapsedMs, tokens, costUsd, pricingSource, toolCalls); the step's time runs from the end of the message before it (question or last tool result) to the assistant message's end; its model is the provider's `response_model`, else the message's `model`, else `ADVISOR_MODEL`. Each assistant message is saved with `{"step": …}` in the `stats` column; `set_stats` merges the reply total beside it on the last message. A new SSE event `step` (`{type, seq, step}`) follows each assistant `message_end`; the `stats` event is unchanged. `GET …/messages` splits the column into `stats` (the total, as before) and a new `step` field, so the tracker's parsing is untouched; rows saved before this read as the total with `step: null`.

**Done differently / notes.**
- Chose a separate `step` event over a field on `message_end`, and a separate `step` field in the API over putting it inside `stats`: the tracker reads any object in `stats` as a reply total, so a step-only object there would have drawn a zero usage line on a stopped reply.
- The reply's `elapsedMs` covers tool calls too, so it is not the sum of the steps' times; tokens, cost and tool calls are.
- A stopped reply's cut-short message gets no step (it has no usage).
- Rust: no code change; added `reply_tests::saved_step_stats_are_passed_over_and_the_usage_line_is_the_replys_total` and a `step` case in `sse_tests::unknown_events_and_bad_data_are_unknown`.
- `crates/bazaar-tracker` `catalog::tests::game_data_db_is_read_from_a_copy_and_the_copy_is_deleted` failed once on the first `cargo test --workspace` (a temp copy still open on Windows) and passed on rerun; unrelated to this change.
- Docs: `advisor.md` (config, warnings, why the old name went, the API, the `step` event, a new Stats section, tests), `history-db.md` and `db-schema.html` (the `stats` column), `INDEX.md`.
