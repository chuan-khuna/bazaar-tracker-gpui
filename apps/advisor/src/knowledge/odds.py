"""Bazaar DB's odds, as the tracker saved them in history.db: the shop odds (``imported_shop_odds``,
the share of each tier among the items merchants offer on a day) and single items' odds
(``imported_item_odds``, the chance an item is in stock at each merchant on a day).

The tracker writes both (crates/bazaar-tracker/src/history/shop_odds.rs and item_odds.rs); the
Advisor only reads them. An item's odds are Bazaar DB's answer kept as it came, read here as
``bazaardb::item_odds::parse_json`` reads it: every field optional, chances from 0 to 1, a day
column such as ``10+`` covering every later day, and a day without enough data not measured.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, replace
from typing import Any

from sqlalchemy import Connection, text

# The tier column of an item the player doesn't own (``NOT_OWNED`` in the tracker).
NOT_OWNED = "None"

TIERS = ("bronze", "silver", "gold", "diamond")


@dataclass(frozen=True, slots=True)
class TierOdds:
    """The shop odds for one day: percent of offered items at each tier."""

    # The day asked for, and the day whose row answers it (the last saved day covers later ones).
    day: int
    row_day: int
    # How many offered items Bazaar DB counted them from.
    samples: int
    percent: dict[str, float]
    # ``bazaardb`` when fetched, ``builtin`` when the tracker seeded its defaults.
    source: str


def tier_odds(connection: Connection, day: int) -> TierOdds | None:
    """The shop odds for ``day``: its own row, else the last saved day before it; None with none."""
    row = connection.execute(
        text(
            "SELECT day, samples, bronze, silver, gold, diamond, source FROM imported_shop_odds "
            "WHERE day <= :day ORDER BY day DESC LIMIT 1"
        ),
        {"day": day},
    ).first()
    if row is None:
        return None
    row_day, samples, *percent, source = row
    return TierOdds(day, row_day, samples, dict(zip(TIERS, (round(p, 1) for p in percent))), source)


@dataclass(frozen=True, slots=True)
class MerchantChance:
    merchant: str
    # In stock after the merchant's one reroll, 0 to 1; None when not measured.
    chance: float | None
    # How often the merchant is presented as an option that day, 0 to 1.
    seen: float | None
    always_tier: str | None


@dataclass(frozen=True, slots=True)
class ItemOdds:
    """One item's odds on one day, for a hero and the tier it is owned at."""

    card_id: str
    item_name: str
    hero: str
    # The tier the saved odds are for: the owned tier, or ``NOT_OWNED``.
    tier: str
    # The day column used (``5``, ``10+``); None when the answer has no column for the day.
    column: str | None
    measured: bool
    # "Any shop": every merchant offered that day, each visited and rerolled.
    any_shop: float | None
    # Best chance first; merchants with no chance that day are left out.
    merchants: tuple[MerchantChance, ...]
    source: str


def _column_of(days: list[str], day: int) -> int | None:
    """``ItemOddsTable::column_of``: the day's own column, or a last ``N+`` column that covers it."""
    for index, label in enumerate(days):
        if label.strip().isdigit() and int(label.strip()) == day:
            return index
    if days:
        last = days[-1].strip()
        if last.endswith("+") and last[:-1].strip().isdigit() and day >= int(last[:-1].strip()):
            return len(days) - 1
    return None


def _truthy(value: Any) -> bool:
    return bool(value) if not isinstance(value, (list, dict)) else True


def _number(value: Any) -> float | None:
    return float(value) if isinstance(value, (int, float)) and not isinstance(value, bool) else None


def read_item_odds(card_id: str, hero: str, tier: str, body: str, day: int, source: str) -> ItemOdds:
    """Bazaar DB's answer ``body`` for one item, on ``day``. A body that isn't JSON reads as no data."""
    try:
        answer = json.loads(body)
    except ValueError:
        answer = {}
    answer = answer if isinstance(answer, dict) else {}
    days = [d if isinstance(d, str) else json.dumps(d) for d in answer.get("days") or []]
    column = _column_of(days, day)
    flags = answer.get("ownedDataByDay") or []
    measured = column is not None and (_truthy(flags[column]) if column < len(flags) else True)

    def at(values: Any) -> float | None:
        if not measured or not isinstance(values, list) or column >= len(values):
            return None
        return _number(values[column])

    merchants = []
    for raw in answer.get("merchants") or []:
        if not isinstance(raw, dict):
            continue
        cells = [cell if isinstance(cell, dict) else {} for cell in raw.get("days") or []]
        chance = at([cell.get("pOfferReroll") for cell in cells])
        if chance:
            merchants.append(
                MerchantChance(
                    merchant=str(raw.get("name") or ""),
                    chance=chance,
                    seen=at([cell.get("pSeen") for cell in cells]),
                    always_tier=raw.get("alwaysTier") if isinstance(raw.get("alwaysTier"), str) else None,
                )
            )
    merchants.sort(key=lambda m: (-(m.chance or 0), m.merchant))
    return ItemOdds(
        card_id=card_id,
        item_name=str(answer.get("itemName") or ""),
        hero=hero,
        tier=tier,
        column=days[column] if column is not None else None,
        measured=measured,
        any_shop=at(answer.get("pAnyRerollByDay")),
        merchants=tuple(merchants),
        source=source,
    )


def item_odds(connection: Connection, card_id: str, hero: str, day: int, tier: str = NOT_OWNED) -> ItemOdds | None:
    """The saved odds for the card (ids and hero ignoring case) on ``day``: those for ``tier`` when
    saved, else for any tier; None when none are saved for this hero."""
    rows = connection.execute(
        text(
            "SELECT template_id, hero, tier, item_name, body, source FROM imported_item_odds "
            "WHERE template_id = lower(:card) AND lower(hero) = lower(:hero) ORDER BY imported_ms DESC"
        ),
        {"card": card_id, "hero": hero},
    ).all()
    if not rows:
        return None
    row = next((r for r in rows if r[2].lower() == tier.lower()), rows[0])
    _, saved_hero, saved_tier, item_name, body, source = row
    odds = read_item_odds(card_id, saved_hero, saved_tier, body, day, source)
    return odds if odds.item_name else replace(odds, item_name=item_name)
