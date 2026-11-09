"""What every Advisor tool shares: the run's situation and sources, the hero, card names to ids, and
the sample's ``n`` and seasons as JSON.

Cards are named by the model as the situation or the card text names them. ``resolve`` matches the
situation's cards first (by id, then name), then the card catalog (``knowledge/cards.py``) by id,
then by name, preferring a card the hero can get.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from agents.advisor.situation import Card, OfferCard, Situation
from agents.advisor.sources import Sources
from knowledge import RunSummary, Sample
from knowledge.cards import NEUTRAL_HERO, CardInfo
from knowledge.stats import size_slots


@dataclass(frozen=True, slots=True)
class ToolContext:
    situation: Situation | None
    sources: Sources


@dataclass(frozen=True, slots=True)
class Resolved:
    """A card the model named, with its id and what is known of it."""

    id: str
    name: str
    size: str | None = None
    tier: str | None = None
    # From the Offer.
    kind: str | None = None
    info: CardInfo | None = None

    @property
    def slots(self) -> int | None:
        return size_slots(self.size)


def hero_of(context: ToolContext, args: dict[str, Any]) -> str:
    """The ``hero`` argument, else the situation's hero; ``ValueError`` with neither."""
    hero = (args.get("hero") or "").strip() or (context.situation.hero if context.situation else None)
    if not hero:
        raise ValueError("No hero: pass 'hero', since the situation doesn't name one.")
    return hero


def text_arg(args: dict[str, Any], key: str) -> str:
    """A string argument that must not be blank, stripped."""
    value = args[key].strip()
    if not value:
        raise ValueError(f"Argument '{key}' must not be empty.")
    return value


def names_arg(args: dict[str, Any], key: str, lowest: int, highest: int) -> list[str]:
    """A list of card names: each a non-blank string, no repeats, ``lowest``-``highest`` of them."""
    names = args[key]
    if not all(isinstance(name, str) and name.strip() for name in names):
        raise ValueError(f"Argument '{key}' must be a list of card names.")
    names = list(dict.fromkeys(name.strip() for name in names))
    if not lowest <= len(names) <= highest:
        raise ValueError(f"Argument '{key}' must have {lowest}-{highest} card names.")
    return names


def int_arg(args: dict[str, Any], key: str, lowest: int, highest: int, default: int | None = None) -> int | None:
    value = args.get(key, default)
    if value is not None and not lowest <= value <= highest:
        raise ValueError(f"Argument '{key}' must be {lowest}-{highest}.")
    return value


def _for_hero(card: CardInfo, hero: str | None) -> bool:
    heroes = {h.lower() for h in card.heroes}
    return hero is None or hero.lower() in heroes or NEUTRAL_HERO.lower() in heroes


def resolve(context: ToolContext, name: str, hero: str | None = None) -> Resolved:
    """The card ``name`` means (an id or a name); ``ValueError`` when nothing matches."""
    key = name.strip().lower()
    catalog = context.sources.catalog
    situation_cards: list[Card] = context.situation.cards() if context.situation else []
    for match in (
        lambda card: card.id is not None and card.id.lower() == key,
        lambda card: card.name.lower() == key,
    ):
        for card in situation_cards:
            if match(card):
                info = catalog.get(card.id) if card.id else _by_name(context, card.name, hero)
                if card.id is None and info is None:
                    continue
                return Resolved(
                    id=card.id or info.id,
                    name=card.name,
                    size=card.size or (info.size if info else None),
                    tier=card.tier,
                    kind=card.kind if isinstance(card, OfferCard) else None,
                    info=info,
                )
    info = catalog.get(key) or _by_name(context, name, hero)
    if info is None:
        hint = "" if len(catalog) else " (no GameData.db was found, so only the situation's cards with an id are known)"
        raise ValueError(f"No card named {name!r}{hint}. Use find_card to look it up.")
    return Resolved(id=info.id, name=info.name, size=info.size, tier=info.starting_tier, info=info)


def _by_name(context: ToolContext, name: str, hero: str | None) -> CardInfo | None:
    found = context.sources.catalog.by_name(name)
    preferred = [card for card in found if _for_hero(card, hero)]
    return (preferred or found or [None])[0]


def name_of(context: ToolContext, card_id: str) -> str:
    """A card id's name: the situation's, else the catalog's, else the id."""
    if context.situation:
        for card in context.situation.cards():
            if card.id and card.id.lower() == card_id.lower():
                return card.name
    info = context.sources.catalog.get(card_id)
    return info.name if info else card_id


def board_cards(context: ToolContext, hero: str) -> list[Resolved]:
    """The situation's Board cards that resolve to an id."""
    found = []
    for card in (context.situation.board or []) if context.situation else []:
        try:
            found.append(resolve(context, card.id or card.name, hero))
        except ValueError:
            continue
    return found


def sample_json(sample: Sample) -> dict[str, Any]:
    """``n``, the seasons and their run counts, the label, and whether it is thin."""
    return {
        "n": sample.n,
        "seasons": [{"season": s.season, "runs": s.runs} for s in sample.seasons],
        "sample": sample.label or "no Scraped runs",
        "thin": sample.thin,
    }


def ratio(part: int, whole: int) -> float | None:
    return round(part / whole, 2) if whole else None


def run_json(context: ToolContext, run: RunSummary) -> dict[str, Any]:
    """One source run as the app shows it on hover and expand."""

    def card(c: Any, board: bool) -> dict[str, Any]:
        out: dict[str, Any] = {"id": c.card_id, "name": name_of(context, c.card_id), "tier": c.tier}
        if board:
            out |= {"slot": c.slot, "size": c.size}
        if c.enchantment:
            out["enchantment"] = c.enchantment
        return out

    return {
        "run_id": run.run_id,
        "hero": run.hero,
        "season": run.season,
        "wins": run.wins,
        "losses": run.losses,
        "victory_tier": run.victory_tier,
        "url": run.url,
        "board": [card(c, True) for c in run.board],
        "skills": [card(c, False) for c in run.skills],
    }
