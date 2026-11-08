"""The season sample: which Scraped runs a hero's numbers come from.

The current season's runs for the hero. When there are fewer than ``min_runs``, the previous
season's are added. Every result names the seasons it used and the runs in each, as
``S20 (8 runs) + S19 (20 runs)``, so advice follows the latest patch and says how thin it is.
"""

from __future__ import annotations

from dataclasses import dataclass

from sqlalchemy import Connection, text

# Under this many runs the sample takes in the previous season too, and is thin.
MIN_RUNS = 10


@dataclass(frozen=True, slots=True)
class SeasonCount:
    season: int
    runs: int


@dataclass(frozen=True, slots=True)
class Sample:
    hero: str
    # Newest first: the current season, then the previous one when it was added.
    seasons: tuple[SeasonCount, ...]
    run_ids: tuple[str, ...]
    min_runs: int = MIN_RUNS

    @property
    def n(self) -> int:
        return len(self.run_ids)

    @property
    def thin(self) -> bool:
        """Fewer runs than ``min_runs``, even with the previous season."""
        return self.n < self.min_runs

    @property
    def label(self) -> str:
        """``S20 (8 runs) + S19 (20 runs)``."""
        return " + ".join(f"S{s.season} ({s.runs} run{'' if s.runs == 1 else 's'})" for s in self.seasons)


def current_season(connection: Connection) -> int | None:
    """The newest season among all imported runs, whatever the hero; None with none."""
    return connection.execute(text("SELECT MAX(season) FROM imported_runs")).scalar_one()


def sample(connection: Connection, hero: str, min_runs: int = MIN_RUNS, season: int | None = None) -> Sample:
    """The runs for ``hero`` (ignoring case) in ``season`` (default: the current one), plus the
    previous season's when that gives fewer than ``min_runs``. Runs with no season are never in it."""
    season = season if season is not None else current_season(connection)
    if season is None:
        return Sample(hero=hero, seasons=(), run_ids=(), min_runs=min_runs)

    def runs_in(which: int) -> list[str]:
        return list(
            connection.execute(
                text(
                    "SELECT run_id FROM imported_runs WHERE lower(hero) = lower(:hero) AND season = :season "
                    "ORDER BY created_at DESC, run_id"
                ),
                {"hero": hero, "season": which},
            ).scalars()
        )

    current = runs_in(season)
    seasons = [SeasonCount(season, len(current))]
    run_ids = list(current)
    if len(current) < min_runs:
        previous = runs_in(season - 1)
        if previous:
            seasons.append(SeasonCount(season - 1, len(previous)))
            run_ids.extend(previous)
    return Sample(hero=hero, seasons=tuple(seasons), run_ids=tuple(run_ids), min_runs=min_runs)
