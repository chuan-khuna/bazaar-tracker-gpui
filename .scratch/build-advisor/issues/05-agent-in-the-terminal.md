# 05: The Advisor agent in the terminal: prompt and first tools

**What to build:** `ADVISOR`, with `prepare()` putting the situation into the prompt, a `prompt.md`, and the tools `find_card`, `rate_options`, `synergy`, `suggest`, `follow_ups` and `ask_user`, all runnable with `just advisor run`. See the spec's Tools and Situation decisions.

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] The situation JSON schema (hero, day, hour, gold, level, health, game state, Board, Stash, skills, the Offer with each card's kind, and the opponent) is documented and validated. `prepare()` renders it into the system prompt.
- [ ] `prompt.md` rules: use only numbers that come from tools, always with `n`; say the data is thin under 10 runs; call `suggest` when ranking an Offer or skills; end with `follow_ups`; use `ask_user` only when the question is unclear.
- [ ] `rate_options` defaults to the Offer. `suggest(title, ranked[{card, reason}])` returns each card's numbers, its source run ids (with Board, skills, result and URL, for the card's hover and expand) and the source line. It doesn't end the reply. `follow_ups(2..3)` doesn't end the reply either.
- [ ] Every tool returns `n` and the seasons, and raises `ValueError` on bad arguments.
- [ ] pytest for each tool on the importer's fixture, and a CLI run with a fake provider that calls `suggest`.
- [ ] By hand: with a real model, the record's example question gives a sensible ranking.
- [ ] Documents: `advisor.md` (the situation, the tools and what each returns, the prompt rules).

## Comments
