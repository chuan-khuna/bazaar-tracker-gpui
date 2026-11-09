"""Loading imported Scraped runs from history.db: as ``RunData`` for the statistics, and as
``RunSummary`` (result, Board, skills, page) for showing where a number comes from."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass

from sqlalchemy import Connection, text

from knowledge.stats import BoardCard, RunData


def load_runs(connection: Connection, run_ids: Iterable[str]) -> list[RunData]:
    """The runs with these ids, in the order given; ids not imported are left out."""
    ids = list(dict.fromkeys(run_ids))
    if not ids:
        return []
    runs: dict[str, RunData] = {}
    # SQLite allows many parameters, but chunk anyway so a big sample never hits the limit.
    for start in range(0, len(ids), 500):
        chunk = ids[start : start + 500]
        marks = ", ".join(f":id{i}" for i in range(len(chunk)))
        params = {f"id{i}": run_id for i, run_id in enumerate(chunk)}
        for run_id, wins, losses in connection.execute(
            text(f"SELECT run_id, wins, losses FROM imported_runs WHERE run_id IN ({marks})"), params
        ):
            runs[run_id] = RunData(run_id=run_id, wins=wins or 0, losses=losses or 0)
        for run_id, card_id, kind, slot, size in connection.execute(
            text(
                f"SELECT run_id, card_id, kind, slot, size FROM imported_run_cards "
                f"WHERE run_id IN ({marks}) ORDER BY run_id, rowid"
            ),
            params,
        ):
            if kind == "board":
                runs[run_id].board.append(BoardCard(card_id, slot, size))
            elif kind == "skill":
                runs[run_id].skills.append(card_id)
        # rowid keeps the file's order, so it is the order the cards were taken in.
        for run_id, card_id in connection.execute(
            text(f"SELECT run_id, card_id FROM imported_picks WHERE run_id IN ({marks}) AND taken = 1 ORDER BY run_id, rowid"),
            params,
        ):
            runs[run_id].taken.append(card_id)
    return [runs[run_id] for run_id in ids if run_id in runs]


@dataclass(frozen=True, slots=True)
class RunCard:
    card_id: str
    tier: str | None
    # Board cards only: the first slot and how many slots it takes.
    slot: int | None
    size: int | None
    enchantment: str | None


@dataclass(frozen=True, slots=True)
class RunSummary:
    """One Scraped run as a source to show: its result, final Board and skills, and its page."""

    run_id: str
    profile_id: str | None
    hero: str
    season: int | None
    wins: int | None
    losses: int | None
    victory_tier: str | None
    url: str | None
    board: tuple[RunCard, ...]
    skills: tuple[RunCard, ...]


def _chunks(ids: list[str]) -> Iterable[tuple[str, dict[str, str]]]:
    for start in range(0, len(ids), 500):
        chunk = ids[start : start + 500]
        yield ", ".join(f":id{i}" for i in range(len(chunk))), {f"id{i}": run_id for i, run_id in enumerate(chunk)}


def run_summaries(connection: Connection, run_ids: Iterable[str]) -> dict[str, RunSummary]:
    """The runs with these ids, by id, in the order given; ids not imported are left out. Board
    cards are in slot order."""
    ids = list(dict.fromkeys(run_ids))
    rows: dict[str, tuple] = {}
    board: dict[str, list[RunCard]] = {run_id: [] for run_id in ids}
    skills: dict[str, list[RunCard]] = {run_id: [] for run_id in ids}
    for marks, params in _chunks(ids):
        for row in connection.execute(
            text(
                "SELECT run_id, profile_id, hero, season, wins, losses, victory_tier, url "
                f"FROM imported_runs WHERE run_id IN ({marks})"
            ),
            params,
        ):
            rows[row[0]] = tuple(row)
        for run_id, card_id, kind, tier, slot, size, enchantment in connection.execute(
            text(
                "SELECT run_id, card_id, kind, tier, slot, size, enchantment FROM imported_run_cards "
                f"WHERE run_id IN ({marks}) ORDER BY run_id, rowid"
            ),
            params,
        ):
            card = RunCard(card_id, tier, slot, size, enchantment)
            (board if kind == "board" else skills)[run_id].append(card)
    found = {}
    for run_id in ids:
        if run_id not in rows:
            continue
        _, profile_id, hero, season, wins, losses, victory_tier, url = rows[run_id]
        placed = sorted(board[run_id], key=lambda card: (card.slot is None, card.slot or 0))
        found[run_id] = RunSummary(
            run_id, profile_id, hero, season, wins, losses, victory_tier, url, tuple(placed), tuple(skills[run_id])
        )
    return found


def profile_count(connection: Connection, run_ids: Iterable[str]) -> int:
    """How many profiles these runs come from."""
    profiles: set[str] = set()
    for marks, params in _chunks(list(dict.fromkeys(run_ids))):
        profiles.update(
            p
            for p in connection.execute(
                text(f"SELECT DISTINCT profile_id FROM imported_runs WHERE run_id IN ({marks})"), params
            ).scalars()
            if p is not None
        )
    return len(profiles)
