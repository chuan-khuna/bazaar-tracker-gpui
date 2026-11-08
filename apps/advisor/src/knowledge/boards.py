"""The queries behind ``shop_plan``, ``arrangement`` and ``similar_runs``, over a hero's season sample:
what strong runs picked on a day, where cards sit on final Boards, and the runs whose final Board
overlaps a given one. Like ``queries``, each returns the sample and the run ids behind each count,
and works from the imported rows (adjacency with ``stats.adjacency_stats``, the formula of
``imported_adjacency_stats``), because a sample can span two seasons.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass

from sqlalchemy import Connection, text

from knowledge.runs import load_runs
from knowledge.sample import MIN_RUNS, Sample, sample
from knowledge.stats import PairStat, RunData, adjacency_stats

# A run with this many wins is a victory: a strong run.
STRONG_WINS = 10


@dataclass(frozen=True, slots=True)
class DayPick:
    """One card among the options on the day, in strong runs: how often offered and taken."""

    card_id: str
    offered: int
    taken: int
    taken_run_ids: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class DayPicks:
    day: int
    # The strong runs in the sample (``STRONG_WINS`` wins or more).
    strong_run_ids: tuple[str, ...]
    # Most taken first.
    picks: tuple[DayPick, ...]
    sample: Sample


def day_picks(connection: Connection, hero: str, day: int, min_runs: int = MIN_RUNS) -> DayPicks:
    """Every card offered on ``day`` in the sample's strong runs, with how often it was offered and
    taken. ``imported_picks`` holds every kind of card (items, skills, encounters): the caller tells
    them apart with the card catalog."""
    found = sample(connection, hero, min_runs)
    runs = load_runs(connection, found.run_ids)
    strong = tuple(run.run_id for run in runs if run.wins >= STRONG_WINS)
    offered: Counter[str] = Counter()
    taken: Counter[str] = Counter()
    taken_in: dict[str, set[str]] = {}
    for start in range(0, len(strong), 500):
        chunk = strong[start : start + 500]
        marks = ", ".join(f":id{i}" for i in range(len(chunk)))
        params = {f"id{i}": run_id for i, run_id in enumerate(chunk)} | {"day": day}
        for run_id, card_id, was_taken in connection.execute(
            text(f"SELECT run_id, card_id, taken FROM imported_picks WHERE day = :day AND run_id IN ({marks})"),
            params,
        ):
            offered[card_id] += 1
            if was_taken:
                taken[card_id] += 1
                taken_in.setdefault(card_id, set()).add(run_id)
    picks = [
        DayPick(card_id, offered[card_id], taken[card_id], tuple(sorted(taken_in.get(card_id, ()))))
        for card_id in offered
    ]
    picks.sort(key=lambda p: (-p.taken, -p.offered, p.card_id))
    return DayPicks(day, strong, tuple(picks), found)


@dataclass(frozen=True, slots=True)
class Position:
    """Where one card sat on the final Boards that have it."""

    card_id: str
    # Runs with it on the final Board (with a slot).
    run_ids: tuple[str, ...]
    # Its first slot (0-9) and how many Boards had it there.
    slots: dict[int, int]
    # How many Boards had nothing to its left, and nothing to its right.
    leftmost: int
    rightmost: int

    @property
    def n(self) -> int:
        return len(self.run_ids)


@dataclass(frozen=True, slots=True)
class Arrangement:
    # Each pair of the given cards side by side on a final Board, once (a before b as given),
    # whatever the support; most runs first.
    pairs: tuple[PairStat, ...]
    # The runs whose final Board has both cards of each pair, keyed as ``pairs``.
    both: dict[tuple[str, str], int]
    positions: tuple[Position, ...]
    sample: Sample


def _position(runs: list[RunData], card_id: str) -> Position:
    slots: Counter[int] = Counter()
    run_ids = []
    leftmost = rightmost = 0
    for run in runs:
        placed = [c for c in run.board if c.slot is not None]
        mine = [c for c in placed if c.card_id == card_id]
        if not mine:
            continue
        run_ids.append(run.run_id)
        card = min(mine, key=lambda c: c.slot)
        slots[card.slot] += 1
        leftmost += card.slot == min(c.slot for c in placed)
        rightmost += card.slot == max(c.slot for c in placed)
    return Position(card_id, tuple(sorted(run_ids)), dict(sorted(slots.items())), leftmost, rightmost)


def arrangement(
    connection: Connection, hero: str, card_ids: tuple[str, ...] | list[str], min_runs: int = MIN_RUNS
) -> Arrangement:
    """For the given Board cards: each pair's runs side by side, and each card's usual position."""
    found = sample(connection, hero, min_runs)
    runs = load_runs(connection, found.run_ids)
    ids = list(dict.fromkeys(card_ids))
    pairs = []
    both = {}
    for i, a in enumerate(ids):
        for b in ids[i + 1 :]:
            stats = [s for s in adjacency_stats(runs, 1, only_a=a) if s.b == b]
            both[(a, b)] = sum(a in run.board_ids and b in run.board_ids for run in runs)
            if stats:
                pairs.append(stats[0])
    pairs.sort(key=lambda s: (-s.n, -(s.lift or 0), ids.index(s.a), ids.index(s.b)))
    return Arrangement(tuple(pairs), both, tuple(_position(runs, card_id) for card_id in ids), found)


@dataclass(frozen=True, slots=True)
class SimilarRun:
    run_id: str
    # The given cards on its final Board.
    shared: tuple[str, ...]
    # ``len(shared)`` over the cards in either Board.
    jaccard: float


@dataclass(frozen=True, slots=True)
class SimilarRuns:
    # Most shared cards first, then the closest Board (Jaccard), then the most wins.
    runs: tuple[SimilarRun, ...]
    sample: Sample


def similar_runs(
    connection: Connection,
    hero: str,
    card_ids: tuple[str, ...] | list[str],
    limit: int = 5,
    min_runs: int = MIN_RUNS,
) -> SimilarRuns:
    """The sample's runs whose final Board shares the most of ``card_ids``; runs sharing none are
    left out."""
    found = sample(connection, hero, min_runs)
    runs = load_runs(connection, found.run_ids)
    mine = list(dict.fromkeys(card_ids))
    scored = []
    for run in runs:
        board = run.board_ids
        shared = tuple(card_id for card_id in mine if card_id in board)
        if shared:
            jaccard = len(shared) / len(board | set(mine))
            scored.append((run, SimilarRun(run.run_id, shared, round(jaccard, 2))))
    scored.sort(key=lambda pair: (-len(pair[1].shared), -pair[1].jaccard, -pair[0].wins, pair[0].run_id))
    return SimilarRuns(tuple(similar for _, similar in scored[:limit]), found)
