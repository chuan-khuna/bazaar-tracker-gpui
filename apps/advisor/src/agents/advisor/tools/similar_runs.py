"""similar_runs: the hero's Scraped runs whose final Board overlaps the player's most, with each run's
result, Board, skills and link."""

from __future__ import annotations

from typing import Any

import knowledge
from agents.advisor.tools.context import ToolContext, board_cards, hero_of, int_arg, names_arg, resolve, run_json, sample_json
from agents.core.tooling import define_tool

MAX_CARDS = 10
DEFAULT_LIMIT = 5


async def _similar_runs(context: ToolContext, args: dict[str, Any]) -> dict[str, Any]:
    hero = hero_of(context, args)
    limit = int_arg(args, "limit", 1, 10, DEFAULT_LIMIT)
    if "cards" in args:
        cards = [resolve(context, name, hero) for name in names_arg(args, "cards", 1, MAX_CARDS)]
    else:
        cards = board_cards(context, hero)
        if not cards:
            raise ValueError("The situation's Board has no known cards: pass 'cards'.")
    cards = list({card.id: card for card in cards}.values())[:MAX_CARDS]
    names = {card.id: card.name for card in cards}
    with context.sources.connection() as connection:
        found = knowledge.similar_runs(connection, hero, [card.id for card in cards], limit=limit)
        summaries = knowledge.run_summaries(connection, [run.run_id for run in found.runs])
    return {
        "hero": hero,
        **sample_json(found.sample),
        "cards": [card.name for card in cards],
        "runs": [
            {
                "shared": [names[card_id] for card_id in run.shared],
                "overlap": len(run.shared),
                "of": len(cards),
                "jaccard": run.jaccard,
                **run_json(context, summaries[run.run_id]),
            }
            for run in found.runs
            if run.run_id in summaries
        ],
    }


def make(context: ToolContext):
    async def run(args: dict[str, Any]) -> dict[str, Any]:
        return await _similar_runs(context, args)

    return define_tool(
        name="similar_runs",
        description=(
            "The hero's Scraped runs whose final Board shares the most cards with yours, best first "
            "(overlap: shared cards of yours; jaccard: shared over all cards in either Board), each "
            "with its wins, losses, victory tier, final Board in slot order, skills and Bazaar DB link "
            "(url). Use it to show what similar builds ended as. 'cards' defaults to the situation's Board."
        ),
        properties={
            "cards": {"type": "array", "items": {"type": "string"}, "description": "card names; default: the Board"},
            "hero": {"type": "string", "description": "default: the situation's hero"},
            "limit": {"type": "integer", "description": f"runs, 1-10, default {DEFAULT_LIMIT}"},
        },
        run=run,
    )
