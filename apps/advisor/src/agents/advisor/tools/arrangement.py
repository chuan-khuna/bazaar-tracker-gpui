"""arrangement: for the Board's cards, which pairs sit side by side on Scraped final Boards, where each
card usually sits, and what the card text says about position. The model proposes the order."""

from __future__ import annotations

import re
from typing import Any

import knowledge
from agents.advisor.tools.context import Resolved, ToolContext, board_cards, hero_of, names_arg, resolve, sample_json
from agents.core.tooling import define_tool

MAX_CARDS = 10

# Card text that cares where a card sits.
POSITION_WORDS = re.compile(r"\b(adjacent|left|right|leftmost|rightmost|neighbou?rs?)\b", re.IGNORECASE)


def mentions_position(text: str | None) -> bool:
    return bool(text) and POSITION_WORDS.search(text) is not None


def _cards(context: ToolContext, args: dict[str, Any], hero: str) -> list[Resolved]:
    if "board" in args:
        cards = [resolve(context, name, hero) for name in names_arg(args, "board", 2, MAX_CARDS)]
    else:
        cards = board_cards(context, hero)
    # One entry per card, however many copies or names.
    cards = list({card.id: card for card in cards}.values())
    if len(cards) < 2:
        where = "'board'" if "board" in args else "The situation's Board"
        raise ValueError(f"{where} has fewer than 2 different known cards: pass 'board' (2-{MAX_CARDS} card names).")
    return cards[:MAX_CARDS]


def _slots_now(context: ToolContext, cards: list[Resolved]) -> dict[str, int]:
    """Each card's first slot on the situation's Board, by id."""
    found: dict[str, int] = {}
    for placed in (context.situation.board or []) if context.situation else []:
        for card in cards:
            same = placed.id.lower() == card.id.lower() if placed.id else placed.name.lower() == card.name.lower()
            if same and placed.slot is not None:
                found.setdefault(card.id, placed.slot)
    return found


async def _arrangement(context: ToolContext, args: dict[str, Any]) -> dict[str, Any]:
    hero = hero_of(context, args)
    cards = _cards(context, args, hero)
    names = {card.id: card.name for card in cards}
    with context.sources.connection() as connection:
        found = knowledge.arrangement(connection, hero, [card.id for card in cards])
    current = _slots_now(context, cards)
    return {
        "hero": hero,
        **sample_json(found.sample),
        "free_slots": context.situation.free_slots() if context.situation else None,
        "pairs": [
            {
                "cards": [names[pair.a], names[pair.b]],
                "ids": [pair.a, pair.b],
                "n": pair.n,
                "of": found.both[(pair.a, pair.b)],
                "lift": round(pair.lift, 2) if pair.lift is not None else None,
                "run_ids": list(pair.run_ids),
            }
            for pair in found.pairs
        ],
        "positions": [
            {
                "card": names[position.card_id],
                "id": position.card_id,
                "size": card.size,
                "slot_now": current.get(card.id),
                "n": position.n,
                "slots": {str(slot): count for slot, count in position.slots.items()},
                "leftmost": position.leftmost,
                "rightmost": position.rightmost,
                "run_ids": list(position.run_ids),
            }
            for card, position in zip(cards, found.positions)
        ],
        "position_text": [
            {"card": card.name, "id": card.id, "text": card.info.text}
            for card in cards
            if card.info is not None and mentions_position(card.info.text)
        ],
    }


def make(context: ToolContext):
    async def run(args: dict[str, Any]) -> dict[str, Any]:
        return await _arrangement(context, args)

    return define_tool(
        name="arrangement",
        description=(
            "Help order the Board. pairs: each pair of the cards side by side on the hero's Scraped "
            "final Boards (n runs side by side, of the runs with both on the Board, lift). positions: "
            "where each card sat (slots: first slot 0-9 to runs; leftmost and rightmost: runs with "
            "nothing to its left or right; n runs with it on the Board) and its slot now. "
            "position_text: card text that mentions adjacent, left or right. You propose the order. "
            "'board' defaults to the situation's Board."
        ),
        properties={
            "board": {"type": "array", "items": {"type": "string"}, "description": "2-10 card names; default: the Board"},
            "hero": {"type": "string", "description": "default: the situation's hero"},
        },
        run=run,
    )
