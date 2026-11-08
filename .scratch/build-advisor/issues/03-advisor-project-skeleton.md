# 03: The Advisor project: apps/advisor, tau-app's core, config, safe history.db access

**What to build:** Create the Python project `apps/advisor` (uv), with tau-app's agent core copied in, the provider config, a module that opens `history.db` safely, and `just/advisor.just`. An empty `ADVISOR` agent should answer in the terminal. See the spec's The Advisor service decisions.

**Blocked by:** 02

**Status:** resolved

- [x] `apps/advisor/pyproject.toml` with fastapi, uvicorn, sqlalchemy, httpx, tau-agent, tau-ai and pytest. Layout: `src/agents/{core,common/tools,advisor}`, `src/knowledge`, `src/importer`, `src/backend`.
- [x] `agents/core` (registry, tooling, provider, cli) and `common/tools/ask_user.py` copied from `D:\code\learning-agent\apps\tau-app`, with their tests.
- [x] Config: `base_url` (default OpenRouter), API key, model, port 8420, and the data folder (`BAZAAR_TRACKER_DATA_DIR`, else `<repo>/.bazaar-tracker`). Changing `base_url` is enough to use a local OpenAI-compatible model.
- [x] A db module opens `history.db` with WAL, `foreign_keys=ON` and `busy_timeout`. It checks `user_version == 9` and refuses otherwise, with a message naming both versions. SQLAlchemy models for the `advisor_*` and `imported_*` tables, with no migrations.
- [x] A `GameData.db` reader that copies the newest file first (as `CardCatalog` does) and looks cards up by id and by name.
- [x] `just advisor sync`, `just advisor run -m "…" [--situation file.json]` and `just advisor test`.
- [x] Tests: the version check, the card lookup on a fixture, and the CLI with a fake provider.
- [x] Documents: a new `docs/documents/advisor.md` (what it is, its layout, config, commands, and the history.db rules), listed in `INDEX.md`. `architecture.md` mentions the third app.

## Comments

Done (not committed). `just advisor sync`, `just advisor test` (37 passed), `cargo build` and `cargo test --workspace` pass.

- **Layout.** As the record's: `src/agents/{core,common/tools,advisor}`, `src/knowledge`, `src/importer`, `src/backend`. Two additions: `src/config.py` (top-level, as the CLI, backend and importer all need it), and the db module in `src/backend/db/` (`connection.py`, `models.py`) as the record places it. The project is built with hatchling from `src/`, so imports have no `src.` prefix and `uv run advisor` is the CLI. `python-dotenv` is added to the dependencies for `apps/advisor/.env`; pytest is in the dev group.
- **Copied from tau-app.** `tooling.py` and `ask_user.py` as they were (docstring only). `registry.py` without tau-app's learning topics, plus `SITUATION_KEY` (the situation goes in the run's state; `ADVISOR.prepare` puts it in the system prompt). `provider.py` reads the config and serves any OpenAI-compatible `base_url`; with no key, a non-OpenRouter endpoint gets no Authorization header. `cli.py` defaults to the `advisor` agent and adds `--situation` and `--fake`. tau-app had no tests of its own for these, so `tests/test_core.py` is new. `MAX_TURNS` is still 12; the record says the Advisor may need more.
- **`--fake`** (not in the ticket): answers with set text through `agents/core/fake.py` and calls no model. There is no API key on this machine, so `just advisor run -m "hi"` was checked with `--fake` (prints the reply) and without it (stops with "No API key for OpenRouter…"); pointing `ADVISOR_BASE_URL` at a closed local port prints `[error] All connection attempts failed`. A real model reply is not checked.
- **Config.** `ADVISOR_BASE_URL`, `ADVISOR_API_KEY` (falls back to `OPENROUTER_API_KEY`), `ADVISOR_MODEL` (tau-app's default, `nvidia/nemotron-3.5-super-vl:free`), `ADVISOR_PORT` (8420), `BAZAAR_TRACKER_DATA_DIR`.
- **history.db.** A missing file is refused and not created (`mode=rw`). The models map `rowid` for the three keyless tables. `message`/`stats` use SQLAlchemy's `JSON` over the TEXT columns. Tests build history.db by running the SQL of every migration parsed out of `schema.rs`, and check each model's columns and nullability against it, so a later migration that touches these tables fails the tests until the models follow.
- **GameData.db.** Looks in the tracker's four places plus `ptr\cache\` (the spec names `{prod,ptr}\cache`). Checked by hand against the real game folder: 3,442 cards loaded from `prod\cache\GameData.db`.
- **By hand.** The repo's own `.bazaar-tracker/history.db` is still at version 8 (the tracker hasn't been run since migration 9), and the Advisor refuses it with "…is at schema version 8, but the Advisor is written for version 9. Start the tracker (`just tracker run`) once to migrate it."
- **Documents.** New `docs/documents/advisor.md`, listed in `INDEX.md`; `architecture.md` (layout, layering, data) and the root `README.md` (recipes, layout) mention the third app.
