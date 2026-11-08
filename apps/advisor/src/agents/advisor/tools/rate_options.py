"""rate_options: each option's numbers, by default the situation's Offer."""

from __future__ import annotations

from typing import Any

import knowledge
from agents.advisor.tools.context import (
    Resolved,
    ToolContext,
    board_cards,
    hero_of,
    int_arg,
    names_arg,
    ratio,
    resolve,
    sample_json,
)
from agents.core.tooling import define_tool
from knowledge import OptionRating

MAX_OPTIONS = 10


def fits(context: ToolContext, card: Resolved) -> bool | None:
    """Whether an item fits in the Board's free slots; None for a skill or encounter, or when the
    Board or a size isn't known."""
    card_type = card.info.card_type if card.info else None
    if card.kind in ("skill", "encounter_choice") or (card_type and card_type != "Item"):
        return None
    free = context.situation.free_slots() if context.situation else None
    return None if free is None or card.slots is None else card.slots <= free


def option_json(
    context: ToolContext, card: Resolved, rating: OptionRating, n: int, board: list[Resolved]
) -> dict[str, Any]:
    """One option's numbers, each with its ``n`` and the run ids behind it."""
    names = {b.id: b.name for b in board}
    return {
        "card": card.name,
        "id": card.id,
        "kind": card.kind,
        "size": card.size,
        "tier": card.tier,
        "fits": fits(context, card),
        "no_data": rating.no_data,
        "runs_with": {"n": rating.n, "of": n, "run_ids": list(rating.run_ids)},
        "picks": {
            "offered": rating.offered,
            "taken": rating.taken,
            "take_rate": ratio(rating.taken, rating.offered),
            "offered_run_ids": list(rating.offered_run_ids),
            "taken_run_ids": list(rating.taken_run_ids),
        },
        "with_board": [
            {
                "card": names.get(fit.board_card_id, fit.board_card_id),
                "id": fit.board_card_id,
                "n": fit.n,
                "of": fit.n_board,
                "lift": round(fit.lift, 2) if fit.lift is not None else None,
                "win_gain": round(fit.win_gain, 3) if fit.win_gain is not None else None,
                "run_ids": list(fit.run_ids),
            }
            for fit in rating.with_board
        ],
    }


def rate(
    context: ToolContext, names: list[str], hero: str, day: int | None = None
) -> tuple[list[Resolved], list[Resolved], knowledge.Ratings]:
    """The cards ``names`` mean and the situation's Board, and the cards rated against that Board over
    the hero's season sample."""
    cards = [resolve(context, name, hero) for name in names]
    board = board_cards(context, hero)
    with context.sources.connection() as connection:
        ratings = knowledge.rate_options(connection, hero, [c.id for c in cards], [b.id for b in board], day=day)
    return cards, board, ratings


async def _rate_options(context: ToolContext, args: dict[str, Any]) -> dict[str, Any]:
    hero = hero_of(context, args)
    day = int_arg(args, "day", 1, 30)
    if "options" in args:
        names = names_arg(args, "options", 1, MAX_OPTIONS)
    else:
        offer = context.situation.offer if context.situation else None
        if not offer:
            raise ValueError("No options given, and the situation has no Offer: pass 'options'.")
        names = [card.id or card.name for card in offer][:MAX_OPTIONS]
    cards, board, ratings = rate(context, names, hero, day)
    n = ratings.sample.n
    return {
        "hero": hero,
        **sample_json(ratings.sample),
        "day": day,
        "options": [option_json(context, card, rating, n, board) for card, rating in zip(cards, ratings.options)],
    }


def make(context: ToolContext):
    async def run(args: dict[str, Any]) -> dict[str, Any]:
        return await _rate_options(context, args)

    return define_tool(
        name="rate_options",
        description=(
            "Numbers for each option from the hero's Scraped runs: the runs whose final Board has it "
            "(runs_with), how often it was offered and taken (picks), and how often it was on a final "
            "Board with each card on the player's Board (with_board: n of the runs with that card, lift), "
            "plus whether it fits in the Board's free slots. Options default to the situation's Offer. "
            "'day' counts only offers on that day. Every count has its n; 'thin' means under 10 runs."
        ),
        properties={
            "options": {"type": "array", "items": {"type": "string"}, "description": "card names; default: the Offer"},
            "hero": {"type": "string", "description": "default: the situation's hero"},
            "day": {"type": "integer"},
        },
        run=run,
    )
