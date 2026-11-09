"""Reading one Scraped run file (``format: 1``, written by apps/scraper's ``bazaar_run_scraper.store``)
into the rows of ``imported_runs``, ``imported_run_cards``, ``imported_picks`` and
``imported_combats``. Pure: no database here.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from knowledge.stats import size_slots

FORMAT = 1


class ScrapedRunError(ValueError):
    """A file that isn't a Scraped run this importer can read."""


@dataclass(slots=True)
class ScrapedRun:
    run: dict[str, Any]
    cards: list[dict[str, Any]] = field(default_factory=list)
    picks: list[dict[str, Any]] = field(default_factory=list)
    combats: list[dict[str, Any]] = field(default_factory=list)
    # card id → the title the file gives it, for every card id the run mentions (to report the ones
    # the card catalog doesn't know).
    titles: dict[str, str] = field(default_factory=dict)

    @property
    def run_id(self) -> str:
        return self.run["run_id"]


def read_file(path: Path) -> ScrapedRun:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, ValueError) as error:
        raise ScrapedRunError(f"not readable JSON: {error}") from error
    return parse(document)


def parse(document: object) -> ScrapedRun:
    """The rows for one Scraped run."""
    if not isinstance(document, dict):
        raise ScrapedRunError("not a JSON object")
    if document.get("format") != FORMAT:
        raise ScrapedRunError(f"format {document.get('format')!r}, not {FORMAT}")
    run_id = document.get("run_id")
    hero = document.get("hero")
    if not isinstance(run_id, str) or not run_id:
        raise ScrapedRunError("no run_id")
    if not isinstance(hero, str) or not hero:
        raise ScrapedRunError("no hero")
    summary = _dict(document.get("summary"))
    scraped = ScrapedRun(
        run={
            "run_id": run_id,
            "profile_id": _str(document.get("profile_id")),
            "hero": hero,
            "season": _int(document.get("season")),
            "created_at": _str(document.get("created_at")),
            "wins": _int(summary.get("statWins")),
            "losses": _int(summary.get("statLosses")),
            "victory_tier": _str(summary.get("victoryTier")),
            "url": _str(document.get("url")),
        }
    )

    for kind, key in (("board", "board"), ("skill", "skills")):
        for card in _list(summary.get(key)):
            card_id = _card_id(card)
            if card_id is None:
                continue
            scraped.titles.setdefault(card_id, _str(card.get("title")) or "")
            scraped.cards.append(
                {
                    "run_id": run_id,
                    "card_id": card_id,
                    "kind": kind,
                    "tier": _str(card.get("tierOverride")),
                    "slot": _int(card.get("slotPosition")) if kind == "board" else None,
                    "size": size_slots(card.get("size")),
                    "enchantment": _str(card.get("enchantmentOverride")),
                }
            )

    timeline = _dict(document.get("timeline"))
    for event in _list(timeline.get("events")):
        if event.get("kind") != "selection":
            continue
        day, hour = _int(event.get("day")), _int(event.get("hour"))
        selected = _dict(event.get("selectedCard"))
        selected_id = _card_id(selected)
        rows = []
        taken_marked = False
        for option in _list(event.get("options")):
            card_id = _card_id(option)
            if card_id is None:
                continue
            # An option offered twice in one event is two rows; only the first is the one taken.
            taken = card_id == selected_id and not taken_marked
            taken_marked = taken_marked or taken
            scraped.titles.setdefault(card_id, _str(option.get("title")) or "")
            rows.append(_pick(run_id, day, hour, card_id, option, taken))
        if selected_id is not None and not taken_marked:
            # Taken but not among the options (some encounter steps): still a pick.
            scraped.titles.setdefault(selected_id, _str(selected.get("title")) or "")
            rows.append(_pick(run_id, day, hour, selected_id, selected, True))
        scraped.picks.extend(rows)

    for combat in _list(summary.get("combats")):
        scraped.combats.append(
            {"run_id": run_id, "day": _int(combat.get("d")), "hour": _int(combat.get("h")), "outcome": _str(combat.get("o"))}
        )
    return scraped


def _pick(run_id: str, day: int | None, hour: int | None, card_id: str, card: dict, taken: bool) -> dict[str, Any]:
    return {"run_id": run_id, "day": day, "hour": hour, "card_id": card_id, "tier": _str(card.get("tierOverride")), "taken": taken}


def _card_id(card: object) -> str | None:
    if not isinstance(card, dict):
        return None
    return _str(card.get("baseId")) or _str(card.get("cardId"))


def _dict(value: object) -> dict:
    return value if isinstance(value, dict) else {}


def _list(value: object) -> list[dict]:
    return [item for item in value if isinstance(item, dict)] if isinstance(value, list) else []


def _str(value: object) -> str | None:
    return value if isinstance(value, str) and value else None


def _int(value: object) -> int | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        try:
            return int(value)
        except ValueError:
            return None
    return None
