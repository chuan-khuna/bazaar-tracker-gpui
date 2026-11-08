# 06: FastAPI and SSE on 127.0.0.1:8420

**What to build:** `just advisor serve` runs FastAPI on localhost:8420 with the spec's API: Conversations by Run, messages, and one streamed reply per message. It copies tau-app's `Runs`, stream and stats. Messages and stats are saved in `advisor_messages`.

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] `GET/POST /api/runs/{run_id}/conversations`, `DELETE /api/conversations/{id}`, `GET /api/conversations/{id}/messages`, and `POST /api/conversations/{id}/messages` with `{text, situation}`, which answers with SSE (`message_start`, `message_end`, `delta` text/thinking/toolcall, `stats`, `error`, `done`).
- [ ] A new Conversation stores its Run, its title (the first words of the first question) and the situation's Day and Hour. An unknown `run_id` gives a 404.
- [ ] One reply at a time per Conversation (409 otherwise). Closing the stream cancels the reply. Messages are saved as each one ends, so a stopped reply keeps the part that arrived.
- [ ] Binds to `127.0.0.1` only. `GET /api/health` gives the version and the `history.db` version.
- [ ] pytest with the test client and a fake provider: event order, saving, stats, cancelling, 404, 409, and refusing a wrong `user_version`.
- [ ] Documents: `advisor.md` (the API, the events and their payloads, `just advisor serve`).

## Comments
