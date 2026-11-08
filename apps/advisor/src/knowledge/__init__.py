"""The numbers the Advisor's tools return: plain functions over history.db and the card catalog, with
no LLM code, so they can be tested with pytest and checked against Bazaar DB.

- ``cards``: the `GameData.db` reader.
- ``sample``: a hero's season sample (the current season, plus the previous one when thin).
- ``queries``: the pair and pick queries over a sample, with ``n``, the seasons and the run ids.
- ``stats``: the formulas (support, lift, win-rate gain with a Beta prior, adjacency, pick order).
"""

from knowledge.queries import card_runs, partners, picked_after, picks, together
from knowledge.sample import MIN_RUNS, Sample, SeasonCount, current_season, sample

__all__ = [
    "MIN_RUNS",
    "Sample",
    "SeasonCount",
    "card_runs",
    "current_season",
    "partners",
    "picked_after",
    "picks",
    "sample",
    "together",
]
