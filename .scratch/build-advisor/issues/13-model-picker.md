# 13: Choose the model from the gpui app, with the default model in Settings

**What to build:** Asked for by the user on 2026-11-08 (screenshot: a model picker in the Advisor tab's input area). The user picks the model in the Advisor tab, beside the input box. The app's Settings store the default model. Each message is answered with the model chosen when it was sent.

**Blocked by:** 11

**Status:** ready-for-agent

- [ ] Backend: `POST /api/conversations/{id}/messages` accepts an optional `model`; when given, that message's reply uses it (same provider and key, `base_url` unchanged), otherwise the config's `ADVISOR_MODEL`. The step stats from ticket 11 record the model actually used. A blank or malformed model gives 400.
- [ ] Backend: `GET /api/models` returns the default model from config and a list to choose from. For OpenRouter, fetch its models list (`GET {base_url}/models`) and cache it for a while; for another `base_url`, try the same OpenAI-compatible `/models` endpoint and fall back to just the default. Failure to fetch still returns the default.
- [ ] App: a model picker in the Advisor tab's input area (under or beside the box, as in the screenshot) showing the current model. Clicking opens a searchable list (reuse the app's existing search/list overlay patterns); a model not in the list can be typed in. The choice is sent with every message.
- [ ] App Settings: a "Default Advisor model" setting, saved with the app's other settings. The picker starts on it; changing the picker for one message doesn't change the default unless the user says so (e.g. a "set as default" action), or pick one clear rule and document it.
- [ ] When the backend can't be reached, the picker still shows the saved default.
- [ ] Tests: pytest for the `model` field (used, defaulted, 400) and `/api/models` (fetched, cached, fallback) with a fake HTTP layer; Rust unit tests for the setting's load/save, the picker's filtering, and the message body carrying the model.
- [ ] Documents: `advisor.md` (the API change, `/api/models`), `gpui-app.md` (the picker, the setting).

## Comments
