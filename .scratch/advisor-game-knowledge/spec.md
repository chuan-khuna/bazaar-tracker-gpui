# Teach the Advisor how to play The Bazaar

Status: ready-for-agent

## Problem

The Advisor's system prompt (`apps/advisor/src/agents/advisor/prompt.md`) says only which tools to call. The model knows nothing reliable about The Bazaar: how a Run goes, what Burn or Haste do, what wins fights, or how each hero is played. Its reasoning from card text, and its reasons for a ranking, rest on whatever it remembers, which is often out of date or wrong.

## Decisions

- **Source:** research first, then draft. The main source is Mobalytics's guides (<https://mobalytics.gg/the-bazaar/guides>), chosen by the user. Other sources only fill gaps. Each skill names its sources and the date they were read, because the game patches often.
- **Delivery:** a short primer in the system prompt, plus **skills**. A skill is `apps/advisor/src/agents/advisor/skills/<name>/SKILL.md`, with front matter (`name`, `description`) and a Markdown body. The prompt lists every skill's name and description, and the model calls `load_skill(name)` to read a body. `load_skill` results are not situation tools, so they stay in the Conversation's history.
- **Scope:** core rules, combat mechanics, general strategy, and per-hero strategy.

## Skills

- `rules`: how a Run goes (days and hours, merchants, events and monsters, the PvP fight, gold and income, health, leveling, prestige and winning), the Board and Stash, card sizes, tiers, enchantments, skills, selling and rerolling.
- `combat`: how a fight runs, and every keyword (cooldowns, Damage, Burn, Poison, Shield, Heal, Regen, Crit, Haste, Slow, Freeze, Charge, Multicast, Ammo, Lifesteal, sandstorm, adjacency, tags).
- `strategy`: general play (economy against tempo, rerolling, building around a core, scaling, pivoting, positioning, common mistakes).
- `hero-<name>`: one per hero, covering their themes, current archetypes, key items and skills, and what to avoid.

## Rules for the prompt

- The primer is short: the core loop, and when to load which skill (load the situation's hero skill and `strategy` before ranking; load `combat` to reason about card text).
- Skills are knowledge, not data. Numbers about Scraped runs still come only from tools, with their `n`.

## Done when

- `load_skill` is in `tools_for` and `TOOL_NAMES`, and the prompt lists the skills.
- Every shipped SKILL.md parses (a test checks it).
- `docs/documents/advisor.md` describes the skills and `load_skill`, and the index line mentions them.
