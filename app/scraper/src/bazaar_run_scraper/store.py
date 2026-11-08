"""The saved runs: a folder per profile, the runs in it by season.

    data/scraper/builds/
      <profile id>/
        metadata.json             the profile: id, name, run count, when it was last read
        stats.json                the profile's stats and card checklist, as Bazaar DB works them out
        index.json                every saved run, newest first, with its season, result and file
        runs/
          season-20/
            <run id>.json         one run
          season-unknown/         runs older than the first season the site knows

A run belongs to the season it was saved in (see `site.Seasons`).

`<run id>.json` is the run as Bazaar DB has it, with the card art left out:

    {
      "format": 1,
      "run_id": "1e69…", "url": "https://bazaardb.gg/run/tracker/1e69…", "kind": "tracker",
      "profile_id": "8297…", "username": "GameplanHQ", "hero": "The Dragons", "season": 20,
      "created_at": "2026-11-07T22:10:31.729989+00:00",
      "summary": { … },
      "timeline": { "snapshots": [ … ], "events": [ … ], "combats": [ … ], "monsterNames": { … }, "player": { … } },
      "saved_ms": 1793534002000
    }

`summary` is the run as the profile lists it: the end of the run (`statWins`, `statLosses`,
`victoryTier`, `statLevel`, `statIncome`, `statGold`, `statMaxHealth`, `statPrestige`), the rank
before and after (`playerRating`, `playerRank`, `playerRankDivision`, `…After`), the final `items`
and `skills` by card id with their attributes, `board` and `skills` named, and `combats`, one per
fight (`d` day, `h` hour, `o` outcome, `p` whether it was against a player).

`timeline` is the run page's: `snapshots` are both boards (`playerBoard`, `playerStash`,
`playerSkills`, `opponentBoard`, …) at each moment of the run; `events` are each choice offered and
taken (`kind` "selection", with `options` and `selectedCard`), each sale (`kind` "sale") and each
board shown (`kind` "board", pointing at a snapshot by `snapshotIndex`); `combats` are the fights,
with the opponent's name, hero, level and rating, and both sides' health after; `monsterNames` names
the monster fought on each `day|hour`. It is null for a run whose page has no timeline.
"""

from __future__ import annotations

import json
import logging
import os
import time
from pathlib import Path
from typing import Any

FORMAT = 1
# Image links and inline pictures: no use for an analysis, and most of each file's size.
DROPPED_KEYS = {"art", "artBlur", "urlFg"}

log = logging.getLogger("scrape-runs")


def clean(value: Any) -> Any:
    """`value` without the card art."""
    if isinstance(value, dict):
        return {key: clean(item) for key, item in value.items() if key not in DROPPED_KEYS}
    if isinstance(value, list):
        return [clean(item) for item in value]
    return value


def season_folder(season: int | None) -> str:
    return f"season-{season:02d}" if season is not None else "season-unknown"


class Store:
    def __init__(self, root: Path, profile_id: str) -> None:
        self.folder = root / profile_id
        self.runs = self.folder / "runs"
        self.runs.mkdir(parents=True, exist_ok=True)

    def path(self, run_id: str, season: int | None) -> Path:
        return self.runs / season_folder(season) / f"{run_id}.json"

    def has(self, run_id: str) -> bool:
        return any(self.runs.glob(f"*/{run_id}.json"))

    def save_run(self, summary: dict, timeline: dict | None, url: str, season: int | None) -> Path:
        path = self.path(summary["id"], season)
        for stale in self.runs.glob(f"*/{summary['id']}.json"):
            if stale != path:
                stale.unlink()  # saved under another season before
        _write(path, {
            "format": FORMAT,
            "run_id": summary["id"],
            "url": url,
            "kind": summary.get("kind"),
            "profile_id": summary.get("profileId"),
            "username": summary.get("username"),
            "hero": summary.get("hero"),
            "season": season,
            "created_at": summary.get("createdAt"),
            "summary": clean(summary),
            "timeline": clean({key: item for key, item in timeline.items() if key != "replayCta"}) if timeline else None,
            "saved_ms": int(time.time() * 1000),
        })
        return path

    def save_profile(self, props: dict, url: str) -> None:
        profile = props.get("profile") or {}
        now = int(time.time() * 1000)
        _write(self.folder / "metadata.json", {
            "format": FORMAT,
            "profile_id": profile.get("id"),
            "profile_name": profile.get("nickname"),
            "url": url,
            "run_count": profile.get("runCount"),
            "verified": profile.get("verified"),
            "is_supporter": profile.get("isSupporter"),
            "current_season": props.get("initialSeason"),
            "saved_ms": now,
        })
        _write(self.folder / "stats.json", {
            "format": FORMAT,
            "profile_id": profile.get("id"),
            "heroes": props.get("allHeroes"),
            "stats": clean(props.get("profileStats")),
            "checklist": clean(props.get("checklistData")),
            "saved_ms": now,
        })

    def write_index(self) -> int:
        """List every saved run in index.json, newest first; returns how many."""
        runs = []
        for path in self.runs.glob("*/*.json"):
            try:
                run = json.loads(path.read_text(encoding="utf-8"))
            except ValueError:
                log.warning(f"store: [path]{path}[/] is not valid JSON; left out of the index")
                continue
            summary = run.get("summary") or {}
            runs.append({
                "run_id": run.get("run_id"),
                "file": path.relative_to(self.folder).as_posix(),
                "season": run.get("season"),
                "hero": run.get("hero"),
                "created_at": run.get("created_at"),
                "wins": summary.get("statWins"),
                "losses": summary.get("statLosses"),
                "victory_tier": summary.get("victoryTier"),
                "rank": summary.get("playerRank"),
                "rating": summary.get("playerRating"),
                "rating_after": summary.get("playerRatingAfter"),
                "unfinished": summary.get("isUnfinished"),
                "has_timeline": run.get("timeline") is not None,
            })
        runs.sort(key=lambda run: run["created_at"] or "", reverse=True)
        _write(self.folder / "index.json", {"format": FORMAT, "runs": runs})
        return len(runs)


def _write(path: Path, document: dict) -> None:
    """Write through a temporary file, so a stopped run never leaves half a file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(document, indent=1, ensure_ascii=False), encoding="utf-8")
    os.replace(temporary, path)
