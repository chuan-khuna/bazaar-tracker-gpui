"""shop_plan: for a day, Bazaar DB's shop tier odds, each wanted item's odds at each merchant, and the
encounters (merchants and events) the strong Scraped runs chose that day."""

from __future__ import annotations

from typing import Any

import knowledge
from agents.advisor.tools.context import Resolved, ToolContext, hero_of, int_arg, name_of, names_arg, resolve, sample_json
from agents.core.tooling import define_tool
from knowledge import NOT_OWNED

MAX_WANTED = 10
MAX_ENCOUNTERS = 8
MAX_MERCHANTS = 5


def is_encounter(card_type: str | None) -> bool:
    """``CombatEncounter``, ``EventEncounter``, ``PedestalEncounter`` (merchants), ``PvpEncounter``,
    … but not ``EncounterStep``, a choice inside an event."""
    return bool(card_type) and card_type.lower().endswith("encounter")


def percent(chance: float | None) -> float | None:
    return round(chance * 100, 1) if chance is not None else None


def owned_tier(context: ToolContext, card: Resolved) -> str:
    """The tier the player owns the card at (Board, then Stash), else ``NOT_OWNED``: the tier
    Bazaar DB's item odds are asked for."""
    situation = context.situation
    for owned in [*(situation.board or ()), *(situation.stash or ())] if situation else ():
        if owned.id and owned.id.lower() == card.id.lower() and owned.tier:
            return owned.tier
    return NOT_OWNED


def _wanted_json(context: ToolContext, card: Resolved, hero: str, day: int, connection: Any) -> dict[str, Any]:
    tier = owned_tier(context, card)
    odds = knowledge.item_odds(connection, card.id, hero, day, tier)
    out: dict[str, Any] = {"card": card.name, "id": card.id, "owned_tier": None if tier == NOT_OWNED else tier}
    if odds is None:
        return out | {"no_odds": True, "note": "No item odds saved for this hero: the tracker saves them when asked."}
    return out | {
        "no_odds": False,
        "odds_tier": None if odds.tier == NOT_OWNED else odds.tier,
        "column": odds.column,
        "measured": odds.measured,
        "any_shop_pct": percent(odds.any_shop),
        "merchants": [
            {"merchant": m.merchant, "chance_pct": percent(m.chance), "seen_pct": percent(m.seen), "always_tier": m.always_tier}
            for m in odds.merchants[:MAX_MERCHANTS]
        ],
        "source": odds.source,
    }


async def _shop_plan(context: ToolContext, args: dict[str, Any]) -> dict[str, Any]:
    hero = hero_of(context, args)
    day = int_arg(args, "day", 1, 30)
    if day is None:
        day = context.situation.day if context.situation else None
        if day is None:
            raise ValueError("No day: pass 'day', since the situation doesn't give one.")
    wanted = [resolve(context, name, hero) for name in names_arg(args, "wanted", 1, MAX_WANTED)] if "wanted" in args else []
    catalog = context.sources.catalog
    with context.sources.connection() as connection:
        tiers = knowledge.tier_odds(connection, day)
        wanted_rows = [_wanted_json(context, card, hero, day, connection) for card in wanted]
        picks = knowledge.day_picks(connection, hero, day)

    encounters = []
    for pick in picks.picks:
        info = catalog.get(pick.card_id)
        if info is not None and is_encounter(info.card_type):
            encounters.append(
                {
                    "card": name_of(context, pick.card_id),
                    "id": pick.card_id,
                    "type": info.card_type,
                    "taken": pick.taken,
                    "offered": pick.offered,
                    "run_ids": list(pick.taken_run_ids),
                }
            )
    result: dict[str, Any] = {"hero": hero, "day": day, **sample_json(picks.sample)}
    result["tier_odds"] = (
        {
            "day": tiers.row_day,
            "samples": tiers.samples,
            **{f"{tier}_pct": value for tier, value in tiers.percent.items()},
            "source": tiers.source,
        }
        if tiers
        else None
    )
    if tiers is None:
        result["tier_odds_note"] = "No shop odds saved for this day: the tracker saves them from Bazaar DB."
    result["wanted"] = wanted_rows
    result["encounters"] = {
        "strong_runs": len(picks.strong_run_ids),
        "rule": f"runs with {knowledge.STRONG_WINS}+ wins",
        "picks": encounters[:MAX_ENCOUNTERS],
    }
    if not len(catalog):
        result["encounters"]["note"] = "No card catalog (GameData.db), so encounters can't be told from other cards."
    return result


def make(context: ToolContext):
    async def run(args: dict[str, Any]) -> dict[str, Any]:
        return await _shop_plan(context, args)

    return define_tool(
        name="shop_plan",
        description=(
            "Plan a day's shops and encounters. tier_odds: Bazaar DB's share of each tier among items "
            "merchants offer that day (percent, from 'samples' offered items). wanted: for each card, "
            "Bazaar DB's chance it is in stock after one reroll at any shop and at the best merchants "
            "(percent), for the tier the player owns it at. encounters: the encounters (merchants, "
            "events, fights) strong Scraped runs were offered that day and how often they took each "
            "(taken of offered; n per count is in strong_runs). 'day' defaults to the situation's."
        ),
        properties={
            "wanted": {"type": "array", "items": {"type": "string"}, "description": "card names the player wants"},
            "hero": {"type": "string", "description": "default: the situation's hero"},
            "day": {"type": "integer", "description": "default: the situation's day"},
        },
        run=run,
    )
