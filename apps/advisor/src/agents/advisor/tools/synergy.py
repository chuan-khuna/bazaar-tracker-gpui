"""synergy: the cards most often on a final Board with the given cards, with lift, runs together and
the mechanics they share."""

from __future__ import annotations

from typing import Any

import knowledge
from agents.advisor.tools.context import ToolContext, hero_of, int_arg, name_of, names_arg, resolve, sample_json
from agents.core.tooling import define_tool
from knowledge.stats import MIN_SUPPORT

MAX_CARDS = 5
DEFAULT_LIMIT = 8


async def _synergy(context: ToolContext, args: dict[str, Any]) -> dict[str, Any]:
    names = names_arg(args, "cards", 1, MAX_CARDS)
    hero = hero_of(context, args)
    limit = int_arg(args, "limit", 1, 20, DEFAULT_LIMIT)
    cards = [resolve(context, name, hero) for name in names]
    catalog = context.sources.catalog
    on_board = {card.id.lower() for card in (context.situation.board or []) if card.id} if context.situation else set()
    results = []
    sample = None
    with context.sources.connection() as connection:
        for card in cards:
            found = knowledge.partners(connection, hero, card.id, limit=limit)
            sample = found.sample
            mine = card.info.mechanics if card.info else frozenset()
            partners = []
            for pair in found.partners:
                info = catalog.get(pair.b)
                partners.append(
                    {
                        "card": name_of(context, pair.b),
                        "id": pair.b,
                        "n": pair.n,
                        "lift": round(pair.lift, 2) if pair.lift is not None else None,
                        "win_gain": round(pair.win_gain, 3) if pair.win_gain is not None else None,
                        "run_ids": list(pair.run_ids),
                        "shared_mechanics": sorted(mine & info.mechanics) if info else [],
                        "on_your_board": pair.b.lower() in on_board,
                    }
                )
            results.append(
                {"card": card.name, "id": card.id, "n": found.n, "run_ids": list(found.run_ids), "partners": partners}
            )
    return {"hero": hero, **sample_json(sample), "min_support": MIN_SUPPORT, "cards": results}


def make(context: ToolContext):
    async def run(args: dict[str, Any]) -> dict[str, Any]:
        return await _synergy(context, args)

    return define_tool(
        name="synergy",
        description=(
            "For each card, its best partners on the hero's Scraped final Boards: runs together (n, with "
            "run ids), lift (above 1: built together more often than chance), win-rate gain, the "
            "mechanics they share (tags and attribute keys from the card text), and whether the partner "
            f"is on the player's Board. Pairs under {MIN_SUPPORT} runs are left out; 'n' per card is the "
            "runs with it, the top-level 'n' the runs looked at."
        ),
        properties={
            "cards": {"type": "array", "items": {"type": "string"}, "description": "card names"},
            "hero": {"type": "string", "description": "default: the situation's hero"},
            "limit": {"type": "integer", "description": f"partners per card, default {DEFAULT_LIMIT}"},
        },
        required=("cards",),
        run=run,
    )
