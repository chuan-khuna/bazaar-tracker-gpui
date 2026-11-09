# 13: Choose the model from the gpui app, with the default model in Settings

**What to build:** Asked for by the user on 2026-11-08 (screenshot: a model picker in the Advisor tab's input area). The user picks the model in the Advisor tab, beside the input box. The app's Settings store the default model. Each message is answered with the model chosen when it was sent.

**Blocked by:** 11

**Status:** resolved

- [x] Backend: `POST /api/conversations/{id}/messages` accepts an optional `model`; when given, that message's reply uses it (same provider and key, `base_url` unchanged), otherwise the config's `ADVISOR_MODEL`. The step stats from ticket 11 record the model actually used. A blank or malformed model gives 400.
- [x] Backend: `GET /api/models` returns the default model from config and a list to choose from. For OpenRouter, fetch its models list (`GET {base_url}/models`) and cache it for a while; for another `base_url`, try the same OpenAI-compatible `/models` endpoint and fall back to just the default. Failure to fetch still returns the default.
- [x] App: a model picker in the Advisor tab's input area (under or beside the box, as in the screenshot) showing the current model. Clicking opens a searchable list (reuse the app's existing search/list overlay patterns); a model not in the list can be typed in. The choice is sent with every message.
- [x] App Settings: a "Default Advisor model" setting, saved with the app's other settings. The picker starts on it; changing the picker for one message doesn't change the default unless the user says so (e.g. a "set as default" action), or pick one clear rule and document it.
- [x] When the backend can't be reached, the picker still shows the saved default.
- [x] Tests: pytest for the `model` field (used, defaulted, 400) and `/api/models` (fetched, cached, fallback) with a fake HTTP layer; Rust unit tests for the setting's load/save, the picker's filtering, and the message body carrying the model.
- [x] Documents: `advisor.md` (the API change, `/api/models`), `gpui-app.md` (the picker, the setting).

## Comments

- 2026-11-08 (agent): Done. Backend: `SendMessage.model` checked by `backend/models.py::checked_model` (blank, malformed, >200 chars or non-string → 400), passed to `Runs.run_message`, which builds the harness and `ReplyStats` with it; `GET /api/models` from `ModelCatalog` (httpx `GET {base_url}/models`, 10-minute cache, a failure keeps the last list or gives the default alone and isn't cached; `--fake` fetches nothing); tests in `tests/test_models.py` with `httpx.MockTransport`. App: `settings.toml` `advisor_model` (empty = the backend's `ADVISOR_MODEL`, no model sent); a model button under the box opening `overlays/model_picker.rs` (word filter on id and name, the backend's-default row, a typed id not in the list, 12 visible rows following the highlight); Settings has a DEFAULT ADVISOR MODEL section (Change… opens the picker in set-default mode, Reset clears). Rule chosen: the picker starts on the saved default; a pick changes only later messages until restart; Shift+Enter (or Shift+click) also saves it as the default. `Request::Create`/`Send` carry the model chosen at send time.
