"""The game's items, read from its own card catalog, as the tracker reads it.

`GameData.db` (SQLite, table `cards`, columns `Id` and `Data`) if one can be read, otherwise
`cards.json` (an object whose values are arrays of cards). Each is looked for in the same four
places as `crates/bazaar-tracker/src/catalog/mod.rs`, taking the copy written most recently.
"""

from __future__ import annotations

import json
import os
import shutil
import sqlite3
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

NEUTRAL_HERO = "Common"


@dataclass(frozen=True)
class Item:
    template_id: str  # lower-cased, as the tracker keys odds
    name: str
    heroes: tuple[str, ...]


def game_folder() -> Path:
    """Where the game keeps `GameData.db` and `cards.json`."""
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support" / "com.TempoStorm.TheBazaar"
    profile = Path(os.environ.get("USERPROFILE", Path.home()))
    return profile / "AppData" / "LocalLow" / "Tempo Storm" / "The Bazaar"


def _candidates(root: Path, file: str) -> list[Path]:
    return [root / "prod" / "cache" / file, root / "cache" / file, root / "prod" / file, root / file]


def _newest(paths: list[Path]) -> Path | None:
    existing = [path for path in paths if path.is_file()]
    return max(existing, key=lambda path: path.stat().st_mtime, default=None)


def _text(value: object) -> str | None:
    if isinstance(value, str):
        return value or None
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return str(value)
    return None


def _item(card_id: str | None, card: dict) -> Item | None:
    if _text(card.get("Type")) != "Item":
        return None
    card_id = card_id or _text(card.get("Id"))
    if not card_id:
        return None
    title = ((card.get("Localization") or {}).get("Title") or {}).get("Text")
    name = _text(title) or _text(card.get("InternalName")) or card_id
    heroes = tuple(hero for hero in (card.get("Heroes") or []) if isinstance(hero, str))
    return Item(template_id=card_id.lower(), name=name, heroes=heroes)


def _from_game_data_db(path: Path) -> list[Item]:
    # The game may be writing it, so read a copy.
    with tempfile.TemporaryDirectory() as folder:
        copy = Path(folder) / "GameData.db"
        shutil.copyfile(path, copy)
        connection = sqlite3.connect(f"file:{copy}?mode=ro", uri=True)
        try:
            rows = connection.execute("SELECT Id, Data FROM cards").fetchall()
        finally:
            connection.close()
    items = []
    for card_id, data in rows:
        try:
            card = json.loads(data)
        except (TypeError, ValueError):
            continue
        if isinstance(card, dict) and (item := _item(_text(card_id), card)):
            items.append(item)
    return items


def _from_cards_json(path: Path) -> list[Item]:
    document = json.loads(path.read_text(encoding="utf-8"))
    items = []
    for cards in document.values() if isinstance(document, dict) else []:
        for card in cards if isinstance(cards, list) else []:
            if isinstance(card, dict) and (item := _item(None, card)):
                items.append(item)
    return items


def load(path: Path | None = None) -> list[Item]:
    """Every item in the catalog at `path` (a `GameData.db` or `cards.json`), or the game's own."""
    if path is not None:
        found = _from_game_data_db(path) if path.suffix == ".db" else _from_cards_json(path)
    else:
        root = game_folder()
        found = []
        if db := _newest(_candidates(root, "GameData.db")):
            try:
                found = _from_game_data_db(db)
            except sqlite3.Error as error:
                print(f"catalog: {db} could not be read ({error}), trying cards.json", file=sys.stderr)
        if not found and (cards := _newest(_candidates(root, "cards.json"))):
            found = _from_cards_json(cards)
        if not found:
            raise SystemExit(f"catalog: no GameData.db or cards.json under {root}; pass --catalog")
    unique = {item.template_id: item for item in found}
    return sorted(unique.values(), key=lambda item: (item.name, item.template_id))


def heroes(items: list[Item]) -> list[str]:
    """Every hero some item is sold to, less the neutral one."""
    return sorted({hero for item in items for hero in item.heroes if hero != NEUTRAL_HERO})


def for_hero(items: list[Item], hero: str, neutral: bool) -> list[Item]:
    """The hero's own items, then (with `neutral`) the neutral ones it does not already have, as
    `item_odds::load_queue` picks them."""
    own = [item for item in items if hero in item.heroes]
    common = [item for item in items if neutral and NEUTRAL_HERO in item.heroes and hero not in item.heroes]
    return own + common
