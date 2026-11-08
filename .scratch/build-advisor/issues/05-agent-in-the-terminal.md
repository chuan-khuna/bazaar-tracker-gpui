# 05: The Advisor agent in the terminal: prompt and first tools

**What to build:** `ADVISOR`, with `prepare()` putting the situation into the prompt, a `prompt.md`, and the tools `find_card`, `rate_options`, `synergy`, `suggest`, `follow_ups` and `ask_user`, all runnable with `just advisor run`. See the spec's Tools and Situation decisions.

**Blocked by:** 04

**Status:** resolved

- [x] The situation JSON schema (hero, day, hour, gold, level, health, game state, Board, Stash, skills, the Offer with each card's kind, and the opponent) is documented and validated. `prepare()` renders it into the system prompt.
- [x] `prompt.md` rules: use only numbers that come from tools, always with `n`; say the data is thin under 10 runs; call `suggest` when ranking an Offer or skills; end with `follow_ups`; use `ask_user` only when the question is unclear.
- [x] `rate_options` defaults to the Offer. `suggest(title, ranked[{card, reason}])` returns each card's numbers, its source run ids (with Board, skills, result and URL, for the card's hover and expand) and the source line. It doesn't end the reply. `follow_ups(2..3)` doesn't end the reply either.
- [x] Every tool returns `n` and the seasons, and raises `ValueError` on bad arguments.
- [x] pytest for each tool on the importer's fixture, and a CLI run with a fake provider that calls `suggest`.
- [ ] By hand: with a real model, the record's example question gives a sensible ranking.
- [x] Documents: `advisor.md` (the situation, the tools and what each returns, the prompt rules).

## Comments

Done on 2026-11-08 (not committed):

- **Situation.** `agents/advisor/situation.py` (pydantic, added to `pyproject.toml`): every field optional, unknown fields refused, cards `{id, name, tier, size, slot, enchantment}`, Offer cards add a required `kind`. `SituationError` names each bad field. `--situation` checks the file and stops on a bad one. `prepare()` renders it as readable lines (hero, day, hour, state; gold, level, health; each list of cards with its kind, size, tier and id; the Board's slots used and free) instead of the raw JSON. If a client skips the check and sends an invalid one, the prompt shows the error and the run still goes ahead.
- **Tools** in `agents/advisor/tools/`. `tools_for(situation, sources)` builds them for each run around the situation and a `Sources` (history.db and the card catalog, opened the first time a tool needs them). Names become ids through the situation's cards first, then `knowledge/cards.py` (by id, then by name, preferring the hero's cards). New `knowledge/` code with no model in it: `rate_options` (one sample load for every option and Board card), `picks(day=)`, `run_summaries`, `profile_count`, and `CardInfo.text`/`tags`/`attributes`/`mechanics` plus `CardCatalog.search`.
- **`suggest`** returns the ranked rows with `rate_options`' numbers, the run ids, a `runs` map (Board, skills, result and URL for every run behind a number) and the `source` line. I added `model_view` to `define_tool` so the model reads the result without `runs` and the run ids, while `details` keeps everything for the app. Ticket 06's backend can pass `details` through as it is.
- **`--fake`** now plays rate_options → suggest → follow_ups → text when the situation has an Offer, so the tools really run against the data folder with no key. Without an Offer it still answers with text only.
- **Tests:** `test_tools.py` (each tool on the record's fixture, plus every tool's `ValueError`s), `test_situation.py`, and `test_cli.py` updated (a fake-provider run that calls `suggest` and carries on, and `--fake` through `main` on a data folder). `just advisor test`: 105 passed.

Done differently, or open:

- **Not done: by-hand check with a real model.** There is no API key on this machine. To do it: put a key in `apps/advisor/.env`, migrate `.bazaar-tracker/history.db` to version 9 by starting the tracker once (it is at 8 now, so the tools say so), run `just advisor import`, then `just advisor run -m "'Which of these should I take?'" --situation <the record's example>`. The advisor.md "The situation" section has the example JSON.
- **Checked instead:** I made a scratch history.db at version 9, imported the 16 real runs into it and ran `--fake` with the record's situation. The tools give Laurel's Bouncy Ball 4/15, offered 15 and taken 7, with Lollipop 3 of 5 (lift 2.25) and Cutter 3 of 4 (lift 2.81), all as in the record. Decanter and Cheese Wheel come out offered 17 times, not 16 and 15, and Cheese Wheel is never on a final Board with Lollipop (the record says once). The run files agree with the tools, so the record's table is off for those two cells. I left the record as written and noted this in advisor.md.
- **"Every tool returns `n` and the seasons":** applies to the four tools that return numbers (find_card, rate_options, synergy, suggest). `follow_ups` and `ask_user` return no numbers, so they have no `n`.
- **Offer kind `skill`:** added beside the spec's three (merchant item, loot, encounter choice) so a level-up skill choice can be ranked, as the prompt rule "ranking an Offer or skills" needs.
- **Fits:** only items are checked against the Board's 10 slots. For skills and encounters, or when the Board or a size isn't known, `fits` is `null`.
- **`just` quoting on Windows** (problem from before this ticket): the PowerShell recipe splits `-m "two words"`. Workaround: `-m "'two words'"`, documented in advisor.md. The recipe itself isn't fixed.
