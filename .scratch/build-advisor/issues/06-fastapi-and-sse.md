# 06: FastAPI and SSE on 127.0.0.1:8420

**What to build:** `just advisor serve` runs FastAPI on localhost:8420 with the spec's API: Conversations by Run, messages, and one streamed reply per message. It copies tau-app's `Runs`, stream and stats. Messages and stats are saved in `advisor_messages`.

**Blocked by:** 05

**Status:** resolved

- [x] `GET/POST /api/runs/{run_id}/conversations`, `DELETE /api/conversations/{id}`, `GET /api/conversations/{id}/messages`, and `POST /api/conversations/{id}/messages` with `{text, situation}`, which answers with SSE (`message_start`, `message_end`, `delta` text/thinking/toolcall, `stats`, `error`, `done`).
- [x] A new Conversation stores its Run, its title (the first words of the first question) and the situation's Day and Hour. An unknown `run_id` gives a 404.
- [x] One reply at a time per Conversation (409 otherwise). Closing the stream cancels the reply. Messages are saved as each one ends, so a stopped reply keeps the part that arrived.
- [x] Binds to `127.0.0.1` only. `GET /api/health` gives the version and the `history.db` version.
- [x] pytest with the test client and a fake provider: event order, saving, stats, cancelling, 404, 409, and refusing a wrong `user_version`.
- [x] Documents: `advisor.md` (the API, the events and their payloads, `just advisor serve`).

## Comments

Done on `feat/item-odds-tier` (not committed).

- **Built:** `apps/advisor/src/backend/`: `main.py` (`create_app`, and `main` for `advisor-serve`, uvicorn on `HOST = "127.0.0.1"` and `ADVISOR_PORT`), `service.py` (history.db opened on first use, the Store and `Runs` on it), `deps.py`, `schemas.py`, `routers/` (health, conversations, messages), `agent/` (tau-app's `runner.py`, `stream.py`, `stats.py`) and `db/store.py` (tau-app's `db/history.py` on `advisor_conversations` and `advisor_messages`). `just advisor serve [--fake]` added; the `advisor-serve` script is in `pyproject.toml`.
- **SSE frames** carry `event: <type>` as well as tau-app's `data: <json>` (the JSON still has `type`), so the app's parser can go by either.
- **Done differently: a wrong `user_version` doesn't stop the service.** The spec's testing section says it "refuses to start"; instead the service starts, logs a warning, `GET /api/health` reports `ok: false` with both versions, and every other route answers 503 with `SchemaVersionError`'s message. history.db is opened again on each request until it opens, so after `just tracker run` migrates the file the Advisor works without a restart. This way the app (ticket 07) can show why rather than "isn't running". Tested in `test_backend.py`.
- **POST `/api/runs/{run_id}/conversations` takes `{text, situation?}`:** the first question (for the title, cut on a word to 48 characters) and the situation (for the Day and Hour). The app then sends the same text to the messages route, as tau-app does.
- **A stopped reply keeps its partial message.** tau-app saves only messages that ended, so text that had streamed into an unfinished assistant message was lost. The runner now saves its text and thinking with `stopReason: "aborted"`, and cancels the harness before closing it so a tool call left without a result is saved with "Tool call interrupted by user". A stopped reply gets no stats row (as in tau-app).
- **Stats** are the provider's usage and cost (`pricingSource: "provider"`): tau-app's OpenRouter price list (`lib/llm/catalog.py`) wasn't copied. OpenRouter reports cost in its stream; a local model reports 0.
- **No `/abort` route** (tau-app has one): closing the stream is the one way to stop, as the spec says.
- **Without a key** (and no `--fake`), `serve` stops at once with the "No API key for OpenRouter…" message plus a hint about `--fake`.
- **Fixed on the way:** `just advisor run -m "two words"` split the message at spaces on Windows. `run` and `import` are now `[positional-arguments]` recipes: on Windows a PowerShell `[script]` that splats `$args` to `uv` (exit code passed on), on Unix `"$@"`. The workaround note in `advisor.md` is replaced.
- **Tests:** `tests/test_backend.py` (17). Cancelling and 409 run the app in uvicorn on a free port, because the test client can't close a stream partway. `just advisor test`: 122 passed.
- **Checked by hand:** `just advisor serve --fake` against the repo's `.bazaar-tracker/history.db`, still at version 8: it listened on 127.0.0.1:8420 only, `/api/health` gave `ok: false`, `version: 8`, `expected: 9` with the migrate hint, `/api/runs/x/conversations` gave 503 with the same message, and the file stayed at 8. Without a key, `just advisor serve` exited 1 with the message.
- **Documents:** `advisor.md` (layout, `serve`, the new "The backend" section: the API, the events and their payloads, one reply at a time, saving, stopping; tests; the quoting fix), `INDEX.md`, `architecture.md`, `history-db.md` (who writes `advisor_*`), and the root `README.md` (`just advisor serve`).
- **Not done:** no by-hand check with a real model (no key used); the app side is ticket 07.
