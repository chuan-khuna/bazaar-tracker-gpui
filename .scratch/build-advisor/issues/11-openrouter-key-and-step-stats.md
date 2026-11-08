# 11: Load the key from OPENROUTER_API_KEY, and save stats for every step

**What to build:** Two changes in `apps/advisor`, asked for by the user on 2026-11-08:

1. The API key comes from `OPENROUTER_API_KEY` (environment or `apps/advisor/.env`). Drop `ADVISOR_API_KEY` so there is one name for the key.
2. A Conversation keeps stats for each step of a reply, not only one total per reply. A step is one model call (one assistant message). Each step saves its token usage (input, output, cache read, cache write), cost, time taken and the model used. The reply total stays as it is.

**Blocked by:** 10

**Status:** ready-for-agent

- [ ] Bug from the user's screenshot: in the gpui app a reply failed with "OpenRouter request failed with status 401 for model nvidia/nemotron-3.5-super-vl:free: Missing Authentication header", while `config.load()` reports a key is set. Find out why `just advisor serve` sent no (or an empty) Authorization header (e.g. `ADVISOR_API_KEY` in `.env` holding a blank or placeholder that wins over `OPENROUTER_API_KEY`, whitespace or quotes not stripped, the server process not loading `.env`, the provider's `omit_authorization_header`), fix it, and add a test. Don't print or log key values; the user's `apps/advisor/.env` is theirs, so don't edit it — say what to change if it needs changing.
- [ ] `config.py` reads the key from `OPENROUTER_API_KEY` only. `.env.example`, the "No API key" message and the docs name it.
- [ ] Each assistant message saved in `advisor_messages` carries its step's stats: model, input, output, cache read and write tokens, cost, and time (ms) from the call's start to its end. The stats of the reply's last message still hold the reply total, alongside its own step.
- [ ] The SSE stream sends each step's stats as the step ends (a documented event or a field on `message_end`), and the final `stats` event keeps its current shape.
- [ ] `GET /api/conversations/{id}/messages` returns each message's step stats, so a reloaded Conversation has them.
- [ ] No schema change in `history.db`: the stats live in the existing `stats` JSON column.
- [ ] pytest: a fake two-step reply (tool call, then text) saves two steps with their tokens, cost, time and model; the total equals the sum; reload returns them; the key comes from `OPENROUTER_API_KEY`.
- [ ] Documents: `advisor.md` (config, the stats and their payloads).

## Comments
