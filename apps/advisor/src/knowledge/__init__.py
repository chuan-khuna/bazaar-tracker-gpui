"""The numbers the Advisor's tools return: plain functions over history.db and the card catalog, with
no LLM code, so they can be tested with pytest and checked against Bazaar DB.

- ``cards``: the `GameData.db` reader.
- ``sample``: a hero's season sample (the current season, plus the previous one when thin).
- ``queries``: the pair and pick queries over a sample, with ``n``, the seasons and the run ids, and
  ``rate_options`` (an Offer's options against the Board).
- ``boards``: what strong runs picked on a day, where cards sit on final Boards, and the runs whose
  final Board overlaps a given one.
- ``odds``: Bazaar DB's shop odds and single items' odds, as the tracker saved them.
- ``runs``: imported runs loaded for the statistics, and as summaries to show as sources.
- ``stats``: the formulas (support, lift, win-rate gain with a Beta prior, adjacency, pick order).
"""

from knowledge.boards import (
    STRONG_WINS,
    Arrangement,
    DayPick,
    DayPicks,
    Position,
    SimilarRun,
    SimilarRuns,
    arrangement,
    day_picks,
    similar_runs,
)
from knowledge.odds import NOT_OWNED, ItemOdds, MerchantChance, TierOdds, item_odds, tier_odds
from knowledge.queries import (
    BoardFit,
    CardRuns,
    OptionRating,
    Ratings,
    card_runs,
    partners,
    picked_after,
    picks,
    rate_options,
    together,
)
from knowledge.runs import RunCard, RunSummary, profile_count, run_summaries
from knowledge.sample import MIN_RUNS, Sample, SeasonCount, current_season, sample

__all__ = [
    "MIN_RUNS",
    "NOT_OWNED",
    "STRONG_WINS",
    "Arrangement",
    "BoardFit",
    "CardRuns",
    "DayPick",
    "DayPicks",
    "ItemOdds",
    "MerchantChance",
    "OptionRating",
    "Position",
    "Ratings",
    "RunCard",
    "RunSummary",
    "Sample",
    "SeasonCount",
    "SimilarRun",
    "SimilarRuns",
    "TierOdds",
    "arrangement",
    "card_runs",
    "current_season",
    "day_picks",
    "item_odds",
    "partners",
    "picked_after",
    "picks",
    "profile_count",
    "rate_options",
    "run_summaries",
    "sample",
    "similar_runs",
    "tier_odds",
    "together",
]
