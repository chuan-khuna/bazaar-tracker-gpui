"""The items the tracker knows, read from the `items.json` it writes into its data folder each time
it loads the game's card catalog (`src/tracking/names.rs`, `write_items_file`):

    { "format": 1, "items": [ { "template_id": "…", "name": "Pasta Maker", "heroes": ["Jules"] } ] }

The data folder is `.bazaar-tracker/` in the repo for a debug build, and `BazaarTracker` in the
system's app data folder for a release build (`src/shell/data_dir.rs`).
"""

from __future__ import annotations

import json
import logging
import os
import sys
from dataclasses import dataclass
from pathlib import Path

NEUTRAL_HERO = "Common"
ITEMS_FILE = "items.json"
REPO = Path(__file__).resolve().parents[4]

log = logging.getLogger("scrape-odds")


@dataclass(frozen=True)
class Item:
    template_id: str  # lower-cased, as the tracker keys odds
    name: str
    heroes: tuple[str, ...]


def data_folders() -> list[Path]:
    """Where the tracker may keep its data: the debug build's folder first, then the release one."""
    if sys.platform == "darwin":
        release = Path.home() / "Library" / "Application Support" / "BazaarTracker"
    elif sys.platform == "win32":
        release = Path(os.environ.get("APPDATA", Path.home())) / "BazaarTracker"
    else:
        release = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local" / "share")) / "BazaarTracker"
    return [REPO / ".bazaar-tracker", release]


def find_items_file() -> Path:
    found = [folder / ITEMS_FILE for folder in data_folders() if (folder / ITEMS_FILE).is_file()]
    if not found:
        looked = ", ".join(str(folder) for folder in data_folders())
        raise SystemExit(f"items: no {ITEMS_FILE} in {looked}. Open the tracker once so it writes one, or pass --items")
    return max(found, key=lambda path: path.stat().st_mtime)


def load(path: Path | None = None) -> list[Item]:
    """Every item in the tracker's `items.json` at `path`, or in its data folder."""
    path = path or find_items_file()
    document = json.loads(path.read_text(encoding="utf-8"))
    if document.get("format") != 1:
        raise SystemExit(f"items: {path} has format {document.get('format')!r}; this scraper reads format 1")
    items = {}
    for raw in document.get("items", []):
        template_id = str(raw.get("template_id") or "").lower()
        if template_id:
            heroes = tuple(hero for hero in raw.get("heroes") or [] if isinstance(hero, str))
            items[template_id] = Item(template_id=template_id, name=str(raw.get("name") or template_id), heroes=heroes)
    log.info(f"items: {len(items)} from [path]{path}[/]")
    return sorted(items.values(), key=lambda item: (item.name, item.template_id))


def heroes(items: list[Item]) -> list[str]:
    """Every hero some item is sold to, less the neutral one."""
    return sorted({hero for item in items for hero in item.heroes if hero != NEUTRAL_HERO})


def for_hero(items: list[Item], hero: str, neutral: bool) -> list[Item]:
    """The hero's own items, then (with `neutral`) the neutral ones it does not already have, as
    `item_odds::load_queue` picks them."""
    own = [item for item in items if hero in item.heroes]
    common = [item for item in items if neutral and NEUTRAL_HERO in item.heroes and hero not in item.heroes]
    return own + common
