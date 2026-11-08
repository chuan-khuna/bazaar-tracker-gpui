"""`just advisor import`: Scraped runs into history.db, then the stats tables.

    uv run advisor-import [--db PATH] [--builds DIR] [--min-support N] [--game-dir DIR]

By default history.db and the builds folder are in the tracker's data folder
(``BAZAAR_TRACKER_DATA_DIR``, else ``<repo>/.bazaar-tracker``).
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import config
from backend.db.connection import HistoryDbError, open_history
from importer.importer import ImportReport, import_builds
from knowledge.cards import CardCatalog
from knowledge.stats import MIN_SUPPORT

BUILDS = Path("raw_data") / "scraper" / "builds"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="advisor-import", description="Import Scraped runs into history.db.")
    parser.add_argument("--db", type=Path, help="history.db (default: the data folder's)")
    parser.add_argument("--builds", type=Path, help=f"the run scraper's folder (default: <data folder>/{BUILDS.as_posix()})")
    parser.add_argument("--min-support", type=int, default=MIN_SUPPORT, help=f"fewest runs a pair needs to be kept (default {MIN_SUPPORT})")
    parser.add_argument("--game-dir", type=Path, help="the game's data folder, for GameData.db (default: found)")
    args = parser.parse_args(argv)

    settings = config.load()
    db = args.db or settings.history_db
    builds = args.builds or settings.data_dir / BUILDS
    if not builds.is_dir():
        print(f"No Scraped runs at {builds}. Scrape some first (`just scraper runs`).", file=sys.stderr)
        return 1
    try:
        engine = open_history(db)
    except HistoryDbError as error:
        print(error, file=sys.stderr)
        return 1
    catalog = CardCatalog.load(args.game_dir)
    try:
        report = import_builds(engine, builds, catalog, args.min_support)
    finally:
        engine.dispose()
    print(describe(report, builds, db, catalog))
    return 0


def describe(report: ImportReport, builds: Path, db: Path, catalog: CardCatalog) -> str:
    lines = [f"Imported {report.imported} runs from {report.files} files in {builds} into {db}."]
    for path, why in report.skipped:
        lines.append(f"  skipped {path}: {why}")
    for (hero, season), counts in report.stats.items():
        lines.append(
            f"  {hero} S{season}: {counts['pair']} pairs, {counts['adjacency']} adjacent pairs, "
            f"{counts['pick']} pick-after-pick pairs"
        )
    if not report.catalog_checked:
        lines.append("No GameData.db found: card ids were not checked against the card catalog.")
    elif report.unknown_cards:
        lines.append(f"{len(report.unknown_cards)} card ids not in {catalog.source} (imported anyway):")
        for unknown in sorted(report.unknown_cards.values(), key=lambda u: (u.title, u.card_id)):
            runs = len(unknown.run_ids)
            lines.append(f"  {unknown.card_id}  {unknown.title or '(no title)'}  in {runs} run{'' if runs == 1 else 's'}")
    else:
        lines.append(f"Every card id is in {catalog.source}.")
    return "\n".join(lines)


if __name__ == "__main__":
    raise SystemExit(main())
