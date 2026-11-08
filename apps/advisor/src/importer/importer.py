"""Importing every Scraped run file into history.db and recomputing the stats tables.

All in one transaction: a run's old rows are deleted (``imported_runs`` and, by cascade and to be
sure, its cards, picks and combats) and its new ones inserted; then the three stats tables are
emptied and worked out again per hero and season from every imported run. Running it twice leaves
the same rows.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

from sqlalchemy import Connection, Engine, delete, insert, select

from backend.db.models import (
    ImportedAdjacencyStats,
    ImportedCombat,
    ImportedPairStats,
    ImportedPick,
    ImportedPickStats,
    ImportedRun,
    ImportedRunCard,
)
from importer.scraped import ScrapedRun, ScrapedRunError, read_file
from knowledge.cards import CardCatalog
from knowledge.runs import load_runs
from knowledge.stats import MIN_SUPPORT, PairStat, adjacency_stats, pair_stats, pick_stats


@dataclass(slots=True)
class UnknownCard:
    """A card id the card catalog doesn't have: still imported, and reported."""

    card_id: str
    # The title the scraped file gives it.
    title: str
    run_ids: set[str] = field(default_factory=set)


@dataclass(slots=True)
class ImportReport:
    files: int = 0
    run_ids: list[str] = field(default_factory=list)
    # (file, why) for each file left out.
    skipped: list[tuple[Path, str]] = field(default_factory=list)
    # By card id. Empty, and ``catalog_checked`` False, when there was no card catalog to check against.
    unknown_cards: dict[str, UnknownCard] = field(default_factory=dict)
    catalog_checked: bool = False
    # (hero, season) → rows written to each stats table.
    stats: dict[tuple[str, int], dict[str, int]] = field(default_factory=dict)

    @property
    def imported(self) -> int:
        return len(self.run_ids)


def find_run_files(builds_dir: Path) -> list[Path]:
    """Every run file: ``<builds>/<profile>/runs/season-NN/<run id>.json`` (and ``season-unknown``)."""
    return sorted(builds_dir.glob("*/runs/season-*/*.json"))


def import_builds(
    engine: Engine,
    builds_dir: Path,
    catalog: CardCatalog | None = None,
    min_support: int = MIN_SUPPORT,
) -> ImportReport:
    """Import every run file under ``builds_dir`` and recompute the stats tables.

    Card ids are checked against ``catalog`` when it has cards; one it doesn't know is still
    imported, and listed in the report's ``unknown_cards``.
    """
    report = ImportReport(catalog_checked=catalog is not None and len(catalog) > 0)
    scraped: dict[str, ScrapedRun] = {}
    for path in find_run_files(builds_dir):
        report.files += 1
        try:
            run = read_file(path)
        except ScrapedRunError as error:
            report.skipped.append((path, str(error)))
            continue
        # The same run in two places (two profiles' folders): the last one read wins.
        scraped[run.run_id] = run

    with engine.begin() as connection:
        for run in scraped.values():
            _replace_run(connection, run)
            report.run_ids.append(run.run_id)
            if report.catalog_checked:
                for card_id, title in run.titles.items():
                    if catalog.get(card_id) is None:
                        unknown = report.unknown_cards.setdefault(card_id, UnknownCard(card_id, title))
                        unknown.run_ids.add(run.run_id)
        report.stats = recompute_stats(connection, min_support)
    return report


def _replace_run(connection: Connection, run: ScrapedRun) -> None:
    for model in (ImportedRunCard, ImportedPick, ImportedCombat, ImportedRun):
        connection.execute(delete(model).where(model.run_id == run.run_id))
    connection.execute(insert(ImportedRun), [run.run])
    for model, rows in ((ImportedRunCard, run.cards), (ImportedPick, run.picks), (ImportedCombat, run.combats)):
        if rows:
            connection.execute(insert(model), rows)


STATS_TABLES = {
    "pair": (ImportedPairStats, pair_stats),
    "adjacency": (ImportedAdjacencyStats, adjacency_stats),
    "pick": (ImportedPickStats, pick_stats),
}


def recompute_stats(connection: Connection, min_support: int = MIN_SUPPORT) -> dict[tuple[str, int], dict[str, int]]:
    """Empty the stats tables and work them out again for each hero and season. Runs with no
    season are left out. Returns the rows written per (hero, season) and table."""
    for model, _ in STATS_TABLES.values():
        connection.execute(delete(model))
    groups: dict[tuple[str, int], list[str]] = defaultdict(list)
    for run_id, hero, season in connection.execute(
        select(ImportedRun.run_id, ImportedRun.hero, ImportedRun.season).where(ImportedRun.season.is_not(None))
    ):
        groups[(hero, season)].append(run_id)
    written: dict[tuple[str, int], dict[str, int]] = {}
    for (hero, season), run_ids in sorted(groups.items()):
        runs = load_runs(connection, run_ids)
        written[(hero, season)] = {}
        for name, (model, compute) in STATS_TABLES.items():
            rows = [_row(hero, season, stat) for stat in compute(runs, min_support)]
            if rows:
                connection.execute(insert(model), rows)
            written[(hero, season)][name] = len(rows)
    return written


def _row(hero: str, season: int, stat: PairStat) -> dict[str, object]:
    return {"hero": hero, "season": season, "a": stat.a, "b": stat.b, "n": stat.n, "lift": stat.lift, "win_gain": stat.win_gain}
