"""find_card: cards in the catalog by name, with what they say and, for the hero, the runs with them."""

from __future__ import annotations

from typing import Any

import knowledge
from agents.advisor.tools.context import ToolContext, sample_json, text_arg
from agents.core.tooling import define_tool
from knowledge.cards import NEUTRAL_HERO

MAX_MATCHES = 8


async def _find_card(context: ToolContext, args: dict[str, Any]) -> dict[str, Any]:
    query = text_arg(args, "query")
    hero_filter = (args.get("hero") or "").strip() or None
    card_type = (args.get("type") or "").strip().lower() or None
    catalog = context.sources.catalog
    if not len(catalog):
        raise ValueError("The card catalog is empty: no GameData.db was found in the game's data folder.")
    matches = catalog.search(query)
    if hero_filter:
        allowed = {hero_filter.lower(), NEUTRAL_HERO.lower()}
        matches = [card for card in matches if allowed & {h.lower() for h in card.heroes}]
    if card_type:
        matches = [card for card in matches if (card.card_type or "").lower() == card_type]
    matches = matches[:MAX_MATCHES]

    # Runs with each card, for the hero asked about or the situation's.
    hero = hero_filter or (context.situation.hero if context.situation else None)
    counts: dict[str, knowledge.CardRuns] = {}
    sample = None
    note = None
    if hero and matches:
        try:
            with context.sources.connection() as connection:
                for card in matches:
                    counts[card.id] = knowledge.card_runs(connection, hero, card.id)
                    sample = counts[card.id].sample
        except ValueError as error:
            note = f"No run counts: {error}"
    result: dict[str, Any] = {"query": query, "hero": hero}
    result |= sample_json(sample) if sample else {"n": 0, "seasons": [], "sample": "no runs counted", "thin": True}
    if note:
        result["note"] = note
    result["matches"] = [
        {
            "card": card.name,
            "id": card.id,
            "type": card.card_type,
            "size": card.size,
            "tier": card.starting_tier,
            "heroes": list(card.heroes),
            "text": card.text,
            "tags": list(card.tags),
            "attributes": card.attributes,
            **(
                {"runs_with": {"n": counts[card.id].n, "of": counts[card.id].sample.n}}
                if card.id in counts
                else {}
            ),
        }
        for card in matches
    ]
    return result


def make(context: ToolContext):
    async def run(args: dict[str, Any]) -> dict[str, Any]:
        return await _find_card(context, args)

    return define_tool(
        name="find_card",
        description=(
            "Look cards up in the game's card catalog by name (or part of one): type, size, starting tier, "
            "heroes, card text, tags and attributes, and for the hero, the Scraped runs whose final Board has "
            "it (runs_with: n of the runs looked at). Use it to reason from card text when there is no data. "
            "Not for the Offer's encounters (merchants, events, fights): those are chosen with shop_plan."
        ),
        properties={
            "query": {"type": "string", "description": "a card name or part of one"},
            "hero": {"type": "string", "description": "only cards this hero can get (and neutral ones)"},
            "type": {"type": "string", "description": "Item, Skill, …"},
        },
        required=("query",),
        run=run,
    )
