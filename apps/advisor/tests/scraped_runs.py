"""Writing Scraped run files as the run scraper does (``format: 1``), and the record's example as a
fixture: 15 Jules runs in which Laurel's Bouncy Ball goes with Giant Lollipop (lift 2.25) and
Pizza Cutter (lift 2.81), and is offered 15 times and taken 7
(docs/records/2026-11-08.001-build-advisor-agent.html, "What the tools return")."""

from __future__ import annotations

import json
from pathlib import Path

PROFILE = "profile-1"

LOLLIPOP = "TPL-GIANT-LOLLIPOP"
CUTTER = "TPL-PIZZA-CUTTER"
BALL = "TPL-BOUNCY-BALL"
DECANTER = "TPL-DECANTER"
CHEESE = "TPL-CHEESE-WHEEL"
SKIP = "TPL-SKIP"

CARDS = {
    LOLLIPOP: ("Giant Lollipop", "Large"),
    CUTTER: ("Pizza Cutter", "Small"),
    BALL: ("Laurel's Bouncy Ball", "Small"),
    DECANTER: ("Decanter", "Medium"),
    CHEESE: ("Cheese Wheel", "Small"),
    SKIP: ("Something Else", "Small"),
}


def card(card_id: str, title: str | None = None, size: str | None = None, tier: str = "Bronze", **extra) -> dict:
    known_title, known_size = CARDS.get(card_id, (card_id, "Small"))
    return {
        "baseId": card_id,
        "tierOverride": tier,
        "enchantmentOverride": None,
        "title": title or known_title,
        "size": size or known_size,
        "type": "Item",
        **extra,
    }


def board(*card_ids: str) -> list[dict]:
    """Cards laid out from slot 0, each after the one before."""
    cards, slot = [], 0
    for card_id in card_ids:
        placed = card(card_id, slotPosition=slot)
        cards.append(placed)
        slot += {"Small": 1, "Medium": 2, "Large": 3}[placed["size"]]
    return cards


def selection(day: int, hour: int, options: list[str], selected: str) -> dict:
    return {"kind": "selection", "day": day, "hour": hour, "selectedCard": card(selected), "options": [card(o) for o in options]}


def run_document(
    run_id: str,
    hero: str = "Jules",
    season: int | None = 20,
    board_ids: tuple[str, ...] = (),
    skill_ids: tuple[str, ...] = (),
    events: list[dict] | None = None,
    wins: int = 10,
    losses: int = 2,
    combats: list[dict] | None = None,
) -> dict:
    return {
        "format": 1,
        "run_id": run_id,
        "url": f"https://bazaardb.gg/run/tracker/{run_id}",
        "kind": "tracker",
        "profile_id": PROFILE,
        "username": "Tester",
        "hero": hero,
        "season": season,
        "created_at": f"2026-11-07T12:00:{len(run_id) % 60:02d}+00:00",
        "summary": {
            "statWins": wins,
            "statLosses": losses,
            "victoryTier": "Gold Victory" if wins >= 10 else None,
            "board": board(*board_ids),
            "skills": [card(s, size="Medium") | {"type": "Skill"} for s in skill_ids],
            "combats": combats if combats is not None else [{"d": 1, "h": 3, "o": "Win", "p": False}],
        },
        "timeline": {"events": [{"kind": "board", "day": 1, "hour": 1}] + (events or [])},
        "saved_ms": 0,
    }


def write_run(builds: Path, document: dict, profile: str = PROFILE) -> Path:
    season = document["season"]
    folder = f"season-{season:02d}" if season is not None else "season-unknown"
    path = builds / profile / "runs" / folder / f"{document['run_id']}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=1), encoding="utf-8")
    return path


# The final Boards of the record's 15 Jules runs (each with a filler card of its own, so the
# fillers make no pairs). Giant Lollipop in 5, Pizza Cutter in 4, the Ball in 4, Decanter in 5,
# Cheese Wheel in 4.
RECORD_BOARDS: list[tuple[str, ...]] = [
    (LOLLIPOP, CUTTER, BALL, DECANTER),
    (LOLLIPOP, CUTTER, BALL),
    (LOLLIPOP, BALL),
    (CUTTER, BALL, DECANTER, CHEESE),
    (LOLLIPOP, DECANTER),
    (LOLLIPOP, CHEESE),
    (CUTTER, DECANTER, CHEESE),
    (DECANTER,),
    (CHEESE,),
    (), (), (), (), (), (),
]

# (card, times offered, times taken): one offer per selection event.
RECORD_PICKS = [(BALL, 15, 7), (DECANTER, 16, 9), (CHEESE, 15, 11)]


def record_run_ids() -> list[str]:
    return [f"jules-{i:02d}" for i in range(1, len(RECORD_BOARDS) + 1)]


def write_record_example(builds: Path) -> list[str]:
    """The record's 15 Jules runs, in S20. Returns their run ids."""
    run_ids = record_run_ids()
    events: dict[str, list[dict]] = {run_id: [] for run_id in run_ids}
    for card_id, offered, taken in RECORD_PICKS:
        for i in range(offered):
            run_id = run_ids[i % len(run_ids)]
            events[run_id].append(selection(1 + i // len(run_ids), 1, [card_id, SKIP], card_id if i < taken else SKIP))
    for i, (run_id, cards) in enumerate(zip(run_ids, RECORD_BOARDS)):
        write_run(builds, run_document(run_id, board_ids=cards + (f"TPL-FILLER-{i}",), events=events[run_id]))
    return run_ids
