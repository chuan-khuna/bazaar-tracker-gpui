# 03: The Advisor project: apps/advisor, tau-app's core, config, safe history.db access

**What to build:** Create the Python project `apps/advisor` (uv), with tau-app's agent core copied in, the provider config, a module that opens `history.db` safely, and `just/advisor.just`. An empty `ADVISOR` agent should answer in the terminal. See the spec's The Advisor service decisions.

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] `apps/advisor/pyproject.toml` with fastapi, uvicorn, sqlalchemy, httpx, tau-agent, tau-ai and pytest. Layout: `src/agents/{core,common/tools,advisor}`, `src/knowledge`, `src/importer`, `src/backend`.
- [ ] `agents/core` (registry, tooling, provider, cli) and `common/tools/ask_user.py` copied from `D:\code\learning-agent\apps\tau-app`, with their tests.
- [ ] Config: `base_url` (default OpenRouter), API key, model, port 8420, and the data folder (`BAZAAR_TRACKER_DATA_DIR`, else `<repo>/.bazaar-tracker`). Changing `base_url` is enough to use a local OpenAI-compatible model.
- [ ] A db module opens `history.db` with WAL, `foreign_keys=ON` and `busy_timeout`. It checks `user_version == 9` and refuses otherwise, with a message naming both versions. SQLAlchemy models for the `advisor_*` and `imported_*` tables, with no migrations.
- [ ] A `GameData.db` reader that copies the newest file first (as `CardCatalog` does) and looks cards up by id and by name.
- [ ] `just advisor sync`, `just advisor run -m "…" [--situation file.json]` and `just advisor test`.
- [ ] Tests: the version check, the card lookup on a fixture, and the CLI with a fake provider.
- [ ] Documents: a new `docs/documents/advisor.md` (what it is, its layout, config, commands, and the history.db rules), listed in `INDEX.md`. `architecture.md` mentions the third app.

## Comments
