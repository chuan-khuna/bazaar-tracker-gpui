"""Statistics over Scraped runs: which cards are built together, sit side by side, and are picked one
after another. Plain functions over plain data, shared by the importer (which stores them per hero
and season in the ``imported_*_stats`` tables) and the queries (which work them out over a season
sample, with the run ids behind each count).

The formulas, for runs ``R`` (``N`` of them) and cards A and B:

- **support** ``n(A,B)``: the runs with both (side by side, or B taken after A, for the other two).
- **lift** ``n(A,B) * N / (n(A) * n(B))``: above 1 means they go together more often than chance.
- **rate** of a set of runs: ``(wins + a) / (wins + losses + a + b)``, a Beta prior worth
  ``PRIOR_GAMES`` games centred on the whole sample's rate, so a handful of runs can't top a list.
- **win-rate gain** of (A, B): ``rate(runs with A and B) - rate(runs with A without B)``; ``None``
  when every run with A has B too. It reads "adding B to A goes with this much more winning".
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass, field
from itertools import combinations

# Pairs with fewer runs than this are dropped from the stats tables and from `partners`. Three keeps
# the record's example (lift 2.25 and 2.81 on 3 runs) while today's data is small; raise it as
# Scraped runs grow.
MIN_SUPPORT = 3

# How many games the Beta prior is worth: about one run.
PRIOR_GAMES = 10.0

# Slots a card takes on the Board, by its size.
SIZE_SLOTS = {"small": 1, "medium": 2, "large": 3}


def size_slots(size: str | None) -> int | None:
    """How many slots a card of ``size`` (Small, Medium, Large) takes; None when not known."""
    return SIZE_SLOTS.get(size.strip().lower()) if isinstance(size, str) else None


@dataclass(frozen=True, slots=True)
class BoardCard:
    card_id: str
    slot: int | None
    size: int | None


@dataclass(slots=True)
class RunData:
    """What the statistics need from one Scraped run."""

    run_id: str
    wins: int = 0
    losses: int = 0
    # The final Board, with slots and sizes.
    board: list[BoardCard] = field(default_factory=list)
    # The final skills' card ids.
    skills: list[str] = field(default_factory=list)
    # The card ids taken from selection events, in the order they were taken.
    taken: list[str] = field(default_factory=list)

    @property
    def basket(self) -> frozenset[str]:
        """The final Board plus skills, as a set of card ids."""
        return frozenset([card.card_id for card in self.board] + self.skills)

    @property
    def board_ids(self) -> frozenset[str]:
        return frozenset(card.card_id for card in self.board)

    def adjacent_pairs(self) -> frozenset[tuple[str, str]]:
        """The pairs of Board cards side by side, both ways round: A ends where B starts."""
        placed = [card for card in self.board if card.slot is not None and card.size is not None]
        pairs = set()
        for left in placed:
            for right in placed:
                if left is not right and left.slot + left.size == right.slot and left.card_id != right.card_id:
                    pairs.add((left.card_id, right.card_id))
                    pairs.add((right.card_id, left.card_id))
        return frozenset(pairs)

    def taken_after_pairs(self) -> frozenset[tuple[str, str]]:
        """The (A, B) where B was taken after A was first taken."""
        first: dict[str, int] = {}
        last: dict[str, int] = {}
        for index, card_id in enumerate(self.taken):
            first.setdefault(card_id, index)
            last[card_id] = index
        return frozenset(
            (a, b) for a, at in first.items() for b, bt in last.items() if a != b and at < bt
        )


@dataclass(frozen=True, slots=True)
class PairStat:
    """One pair's numbers: ``n`` runs, with their ids."""

    a: str
    b: str
    n: int
    lift: float | None
    win_gain: float | None
    run_ids: tuple[str, ...]


class Prior:
    """The Beta prior for a sample: ``PRIOR_GAMES`` games at the sample's own win rate."""

    def __init__(self, runs: Iterable[RunData], games: float = PRIOR_GAMES) -> None:
        wins = losses = 0
        for run in runs:
            wins += run.wins
            losses += run.losses
        mean = wins / (wins + losses) if wins + losses else 0.5
        self.alpha = games * mean
        self.beta = games * (1 - mean)

    def rate(self, runs: Iterable[RunData]) -> float:
        wins = losses = 0
        for run in runs:
            wins += run.wins
            losses += run.losses
        return (wins + self.alpha) / (wins + losses + self.alpha + self.beta)


def lift(together: int, n_a: int, n_b: int, total: int) -> float | None:
    """``n(A,B) * N / (n(A) * n(B))``; None when A or B is in no run."""
    return together * total / (n_a * n_b) if n_a and n_b else None


def _win_gain(prior: Prior, with_both: list[RunData], with_a_only: list[RunData]) -> float | None:
    if not with_both or not with_a_only:
        return None
    return prior.rate(with_both) - prior.rate(with_a_only)


def _stats(
    runs: Sequence[RunData],
    singles: Mapping[str, frozenset[str]],
    pairs: Mapping[str, frozenset[tuple[str, str]]],
    min_support: int,
    only_a: str | None = None,
    only_b: str | None = None,
) -> list[PairStat]:
    """Every ordered pair (A, B) in ``pairs`` with at least ``min_support`` runs.

    ``singles[run_id]`` are the cards counted for ``n(A)``; ``pairs[run_id]`` the ordered pairs
    the run has. ``only_a``/``only_b`` keep to pairs with that A or B.
    """
    total = len(runs)
    n_single: Counter[str] = Counter()
    runs_with: dict[str, list[RunData]] = {}
    for run in runs:
        for card_id in singles[run.run_id]:
            n_single[card_id] += 1
            runs_with.setdefault(card_id, []).append(run)
    together: dict[tuple[str, str], list[RunData]] = {}
    for run in runs:
        for pair in pairs[run.run_id]:
            if (only_a is None or pair[0] == only_a) and (only_b is None or pair[1] == only_b):
                together.setdefault(pair, []).append(run)
    prior = Prior(runs)
    found = []
    for (a, b), both in together.items():
        if len(both) < min_support:
            continue
        both_ids = {run.run_id for run in both}
        a_only = [run for run in runs_with.get(a, ()) if run.run_id not in both_ids]
        found.append(
            PairStat(
                a=a,
                b=b,
                n=len(both),
                lift=lift(len(both), n_single[a], n_single[b], total),
                win_gain=_win_gain(prior, both, a_only),
                run_ids=tuple(sorted(both_ids)),
            )
        )
    found.sort(key=lambda stat: (-stat.n, -(stat.lift or 0), stat.a, stat.b))
    return found


def _both_ways(cards: frozenset[str]) -> frozenset[tuple[str, str]]:
    return frozenset(pair for a, b in combinations(sorted(cards), 2) for pair in ((a, b), (b, a)))


def pair_stats(
    runs: Sequence[RunData], min_support: int = MIN_SUPPORT, *, only_a: str | None = None, only_b: str | None = None
) -> list[PairStat]:
    """Cards on the same final Board or skills, both ways round (lift and ``n`` are the same both
    ways; the win-rate gain is not)."""
    baskets = {run.run_id: run.basket for run in runs}
    pairs = {run.run_id: _both_ways(run.basket) for run in runs}
    return _stats(runs, baskets, pairs, min_support, only_a, only_b)


def adjacency_stats(
    runs: Sequence[RunData], min_support: int = MIN_SUPPORT, *, only_a: str | None = None
) -> list[PairStat]:
    """Board cards side by side (from slot and size), both ways round. ``n(A)`` counts Boards."""
    boards = {run.run_id: run.board_ids for run in runs}
    pairs = {run.run_id: run.adjacent_pairs() for run in runs}
    return _stats(runs, boards, pairs, min_support, only_a)


def pick_stats(
    runs: Sequence[RunData], min_support: int = MIN_SUPPORT, *, only_a: str | None = None
) -> list[PairStat]:
    """B taken after A. ``n(A)`` counts the runs that took A."""
    taken = {run.run_id: frozenset(run.taken) for run in runs}
    pairs = {run.run_id: run.taken_after_pairs() for run in runs}
    return _stats(runs, taken, pairs, min_support, only_a)
