"""The pair and pick queries the Advisor's tools call. Each works over a hero's season sample and
returns ``n``, the sample (its seasons and run counts), and the run ids behind each count.

They are worked out from the imported rows each time, with the same formulas as the stats tables
(``knowledge.stats``), because a sample can span two seasons while the tables hold one season each.
"""

from __future__ import annotations

from dataclasses import dataclass

from sqlalchemy import Connection, text

from knowledge.runs import load_runs
from knowledge.sample import MIN_RUNS, Sample, sample
from knowledge.stats import MIN_SUPPORT, PairStat, RunData, lift, pair_stats, pick_stats


@dataclass(frozen=True, slots=True)
class CardRuns:
    """The runs in the sample whose final Board or skills have the card."""

    card_id: str
    n: int
    run_ids: tuple[str, ...]
    sample: Sample


@dataclass(frozen=True, slots=True)
class Partners:
    """The cards most often built with ``card_id``, best first, each with at least ``min_support`` runs."""

    card_id: str
    # Runs with the card.
    n: int
    run_ids: tuple[str, ...]
    partners: tuple[PairStat, ...]
    sample: Sample


@dataclass(frozen=True, slots=True)
class Together:
    """Two cards on the same final Board or skills, whatever the support."""

    pair: PairStat
    sample: Sample


@dataclass(frozen=True, slots=True)
class Picks:
    """How often the card was offered and taken in selection events. One offer seen in two events
    counts twice."""

    card_id: str
    offered: int
    taken: int
    # The runs it was offered in, and the runs it was taken in.
    run_ids: tuple[str, ...]
    taken_run_ids: tuple[str, ...]
    sample: Sample


@dataclass(frozen=True, slots=True)
class PickedAfter:
    """The cards most often taken after ``card_id`` in the same run, best first."""

    card_id: str
    # Runs that took the card.
    n: int
    run_ids: tuple[str, ...]
    after: tuple[PairStat, ...]
    sample: Sample


def _runs(connection: Connection, hero: str, min_runs: int) -> tuple[Sample, list[RunData]]:
    found = sample(connection, hero, min_runs)
    return found, load_runs(connection, found.run_ids)


def card_runs(connection: Connection, hero: str, card_id: str, min_runs: int = MIN_RUNS) -> CardRuns:
    found, runs = _runs(connection, hero, min_runs)
    ids = tuple(sorted(run.run_id for run in runs if card_id in run.basket))
    return CardRuns(card_id=card_id, n=len(ids), run_ids=ids, sample=found)


def partners(
    connection: Connection,
    hero: str,
    card_id: str,
    limit: int | None = 10,
    min_support: int = MIN_SUPPORT,
    min_runs: int = MIN_RUNS,
) -> Partners:
    found, runs = _runs(connection, hero, min_runs)
    ids = tuple(sorted(run.run_id for run in runs if card_id in run.basket))
    stats = pair_stats(runs, min_support, only_a=card_id)
    return Partners(card_id, len(ids), ids, tuple(stats[:limit] if limit is not None else stats), found)


def together(connection: Connection, hero: str, a: str, b: str, min_runs: int = MIN_RUNS) -> Together:
    found, runs = _runs(connection, hero, min_runs)
    stats = pair_stats(runs, 1, only_a=a, only_b=b)
    if stats:
        return Together(stats[0], found)
    n_a = sum(a in run.basket for run in runs)
    n_b = sum(b in run.basket for run in runs)
    return Together(PairStat(a, b, 0, lift(0, n_a, n_b, len(runs)), None, ()), found)


def picks(connection: Connection, hero: str, card_id: str, min_runs: int = MIN_RUNS) -> Picks:
    found = sample(connection, hero, min_runs)
    offered = taken = 0
    offered_in: set[str] = set()
    taken_in: set[str] = set()
    ids = list(found.run_ids)
    for start in range(0, len(ids), 500):
        chunk = ids[start : start + 500]
        marks = ", ".join(f":id{i}" for i in range(len(chunk)))
        params = {f"id{i}": run_id for i, run_id in enumerate(chunk)} | {"card": card_id}
        for run_id, was_taken in connection.execute(
            text(f"SELECT run_id, taken FROM imported_picks WHERE card_id = :card AND run_id IN ({marks})"), params
        ):
            offered += 1
            offered_in.add(run_id)
            if was_taken:
                taken += 1
                taken_in.add(run_id)
    return Picks(card_id, offered, taken, tuple(sorted(offered_in)), tuple(sorted(taken_in)), found)


def picked_after(
    connection: Connection,
    hero: str,
    card_id: str,
    limit: int | None = 10,
    min_support: int = MIN_SUPPORT,
    min_runs: int = MIN_RUNS,
) -> PickedAfter:
    found, runs = _runs(connection, hero, min_runs)
    ids = tuple(sorted(run.run_id for run in runs if card_id in run.taken))
    stats = pick_stats(runs, min_support, only_a=card_id)
    return PickedAfter(card_id, len(ids), ids, tuple(stats[:limit] if limit is not None else stats), found)
