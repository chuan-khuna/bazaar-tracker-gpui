"""shop_plan: for a day, Bazaar DB's shop tier odds, each wanted item's odds at each merchant, the
encounters (merchants and events) the strong Scraped runs chose that day, and, when the Offer is a
choice of encounters, each offered one with how often strong runs took it and the wanted cards it
sells. Encounters are not cards: this is the tool for choosing one."""

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


def _wanted_json(card: Resolved, tier: str, odds: knowledge.ItemOdds | None) -> dict[str, Any]:
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
        wanted_odds = []
        for card in wanted:
            tier = owned_tier(context, card)
            wanted_odds.append((card, tier, knowledge.item_odds(connection, card.id, hero, day, tier)))
        picks = knowledge.day_picks(connection, hero, day)
    wanted_rows = [_wanted_json(card, tier, odds) for card, tier, odds in wanted_odds]

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
    offered = _offered_json(context, picks, wanted_odds)
    if offered:
        result["offered"] = offered
        result["offered_note"] = (
            "The encounters on offer now. strong_runs: how often strong runs took it when offered that day "
            "(null: not offered to them). sells_wanted: each wanted card's chance at this merchant after "
            "one reroll; a wanted card with odds that is missing here has no chance at it that day (or "
            "it isn't a merchant)."
        )
    return result


def _offered_json(
    context: ToolContext, picks: knowledge.DayPicks, wanted_odds: list[tuple[Resolved, str, knowledge.ItemOdds | None]]
) -> list[dict[str, Any]]:
    """The situation's Offer of encounters: each with its strong-run picks that day and the wanted
    cards it sells, matched to Bazaar DB's merchants by name."""
    offer = context.situation.offer if context.situation else None
    by_id = {pick.card_id.lower(): pick for pick in picks.picks}
    out = []
    for card in offer or ():
        if card.kind != "encounter_choice":
            continue
        pick = by_id.get(card.id.lower()) if card.id else None
        sells = []
        for wanted, _tier, odds in wanted_odds:
            if odds is None:
                continue
            for merchant in odds.merchants:
                if merchant.merchant.strip().lower() == card.name.strip().lower():
                    sells.append({"card": wanted.name, "chance_pct": percent(merchant.chance), "always_tier": merchant.always_tier})
        out.append(
            {
                "encounter": card.name,
                "id": card.id,
                "strong_runs": {"taken": pick.taken, "offered": pick.offered, "run_ids": list(pick.taken_run_ids)} if pick else None,
                "sells_wanted": sells,
            }
        )
    return out


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
            "(taken of offered; n per count is in strong_runs). offered: when the Offer is a choice of "
            "encounters, each one with how often strong runs took it and which wanted cards it sells. "
            "Use this, not find_card or rate_options, to choose an encounter: encounters are not cards. "
            "'day' defaults to the situation's."
        ),
        properties={
            "wanted": {"type": "array", "items": {"type": "string"}, "description": "card names the player wants"},
            "hero": {"type": "string", "description": "default: the situation's hero"},
            "day": {"type": "integer", "description": "default: the situation's day"},
        },
        run=run,
    )
