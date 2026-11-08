"""The situation: what the app sends with every message about the Run being played, checked and
rendered into the system prompt.

Every field may be left out (the app sends what the game has shown so far), but a field that is
there must have the right type, and unknown fields are refused so a misspelt key is caught. Cards
carry their `GameData.db` id when the app knows it; the tools match cards by that id, else by name.
The schema is documented in docs/documents/advisor.md ("The situation").
"""

from __future__ import annotations

from typing import Annotated, Literal, get_args

from pydantic import BaseModel, ConfigDict, Field, ValidationError, field_validator

from knowledge.stats import size_slots

# Slots on a Board.
BOARD_SLOTS = 10

# What an Offer's card is: a merchant's item, loot after a fight, a choice of encounters, or a skill
# (from a level-up or an encounter).
OfferKind = Literal["merchant_item", "loot", "encounter_choice", "skill"]
OFFER_KINDS: tuple[str, ...] = get_args(OfferKind)

Size = Literal["Small", "Medium", "Large"]


class SituationError(ValueError):
    """The situation isn't valid: the message names each bad field."""


class _Strict(BaseModel):
    model_config = ConfigDict(extra="forbid", str_strip_whitespace=True)


class Card(_Strict):
    # The card's `GameData.db` id (the Snapshot's template_id).
    id: str | None = None
    name: Annotated[str, Field(min_length=1)]
    tier: str | None = None
    size: Size | None = None
    # Board cards: the first slot it takes, 0-9.
    slot: Annotated[int, Field(ge=0, lt=BOARD_SLOTS)] | None = None
    enchantment: str | None = None

    @field_validator("size", mode="before")
    @classmethod
    def _size_any_case(cls, value: object) -> object:
        return value.strip().capitalize() if isinstance(value, str) else value

    @property
    def slots(self) -> int | None:
        """How many Board slots it takes; None when its size isn't known."""
        return size_slots(self.size)


class OfferCard(Card):
    kind: OfferKind


class Opponent(_Strict):
    name: str | None = None
    hero: str | None = None
    health: int | None = None
    health_max: int | None = None
    board: list[Card] = []
    skills: list[Card] = []


class Situation(_Strict):
    hero: str | None = None
    day: Annotated[int, Field(ge=1)] | None = None
    hour: Annotated[int, Field(ge=0)] | None = None
    gold: int | None = None
    level: int | None = None
    health: int | None = None
    health_max: int | None = None
    # The game's state in words: Shop, Encounter choice, Combat, Loot, …
    game_state: str | None = None
    # None: not sent (the Board isn't known); [] an empty Board.
    board: list[Card] | None = None
    stash: list[Card] | None = None
    skills: list[Card] | None = None
    offer: list[OfferCard] | None = None
    # Only in a combat.
    opponent: Opponent | None = None

    def cards(self) -> list[Card]:
        """Every card it mentions: Board, Stash, skills, Offer, then the opponent's."""
        found: list[Card] = [*(self.board or ()), *(self.stash or ()), *(self.skills or ()), *(self.offer or ())]
        if self.opponent:
            found += [*self.opponent.board, *self.opponent.skills]
        return found

    def free_slots(self) -> int | None:
        """Board slots left; None when the Board wasn't sent or a card's size isn't known."""
        if self.board is None:
            return None
        used = [card.slots for card in self.board]
        return None if None in used else BOARD_SLOTS - sum(used)


def parse_situation(value: object) -> Situation:
    """The situation in ``value`` (the app's JSON object), or ``SituationError`` naming what's wrong."""
    if isinstance(value, Situation):
        return value
    try:
        return Situation.model_validate(value)
    except ValidationError as error:
        problems = []
        for item in error.errors():
            where = ".".join(str(part) for part in item["loc"]) or "situation"
            problems.append(f"{where}: {item['msg']}")
        raise SituationError("The situation isn't valid: " + "; ".join(problems)) from None


def _card_line(card: Card) -> str:
    details = [part for part in (card.size, card.tier, card.enchantment) if part]
    if isinstance(card, OfferCard):
        details.insert(0, card.kind.replace("_", " "))
    text = card.name + (f" ({', '.join(details)})" if details else "")
    return text + (f" [id {card.id}]" if card.id else "")


def _cards(title: str, cards: list[Card] | None, board: bool = False) -> list[str]:
    if cards is None:
        return [f"{title}: not sent."]
    if not cards:
        return [f"{title}: empty."]
    lines = [f"{title}:"]
    for card in cards:
        where = f"slot {card.slot}: " if board and card.slot is not None else ""
        lines.append(f"- {where}{_card_line(card)}")
    return lines


def render_situation(situation: Situation) -> str:
    """The situation as the prompt's ``## Situation`` section: a line of numbers, then each list of cards."""
    head = []
    if situation.hero:
        head.append(f"Hero: {situation.hero}")
    if situation.day is not None:
        head.append(f"Day {situation.day}" + (f", Hour {situation.hour}" if situation.hour is not None else ""))
    if situation.game_state:
        head.append(situation.game_state)
    numbers = []
    if situation.gold is not None:
        numbers.append(f"Gold {situation.gold}")
    if situation.level is not None:
        numbers.append(f"Level {situation.level}")
    if situation.health is not None:
        numbers.append(
            f"Health {situation.health}" + (f"/{situation.health_max}" if situation.health_max is not None else "")
        )
    lines = ["## Situation", ""]
    if head:
        lines.append(" · ".join(head))
    if numbers:
        lines.append(" · ".join(numbers))
    lines.append("")
    board_title = "Board"
    free = situation.free_slots()
    if free is not None:
        board_title += f" ({BOARD_SLOTS - free} of {BOARD_SLOTS} slots used, {free} free)"
    lines += _cards(board_title, situation.board, board=True)
    lines += _cards("Stash", situation.stash)
    lines += _cards("Skills", situation.skills)
    lines += _cards("Offer", situation.offer)
    if situation.opponent:
        them = situation.opponent
        who = " · ".join(
            part
            for part in (
                them.name,
                them.hero,
                f"Health {them.health}" + (f"/{them.health_max}" if them.health_max is not None else "")
                if them.health is not None
                else None,
            )
            if part
        )
        lines.append("")
        lines.append("Opponent" + (f": {who}" if who else ""))
        lines += _cards("Opponent's Board", them.board, board=True)
        lines += _cards("Opponent's skills", them.skills)
    return "\n".join(lines).rstrip()
