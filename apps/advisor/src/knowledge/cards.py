"""The card catalog: cards by id and by name, from the game's own `GameData.db`.

As the tracker's ``CardCatalog`` (crates/bazaar-tracker/src/catalog/), the newest `GameData.db`
under the game's data folder is copied to a temporary file and read from the copy, because the
game may be running and writing it. The copy is deleted afterwards. Finding nothing is not an
error: the catalog is empty. Scraped runs' ``baseId``/``cardId`` are these ids.
"""

from __future__ import annotations

import json
import os
import shutil
import sqlite3
import sys
import tempfile
from collections.abc import Iterable, Iterator
from dataclasses import dataclass, field
from pathlib import Path

# What the game writes in a card's Heroes when every hero can get it.
NEUTRAL_HERO = "Common"

# Attribute keys nearly every card has, which say nothing about what it works with.
GENERIC_KEYS = frozenset({"CooldownMax", "Multicast", "BuyPrice", "SellPrice"})


@dataclass(frozen=True, slots=True)
class CardInfo:
    # As the game writes it.
    id: str
    # Its localised title, else its internal name, else its id.
    name: str
    starting_tier: str | None = None
    size: str | None = None
    card_type: str | None = None
    description: str | None = None
    art_key: str | None = None
    # The heroes whose shops sell it; a neutral card has NEUTRAL_HERO.
    heroes: tuple[str, ...] = ()
    # The card's whole JSON, for what the fields above leave out (tags, attributes, tiers).
    data: dict[str, object] = field(default_factory=dict, repr=False, compare=False)

    @property
    def text(self) -> str | None:
        """What the card says: its description, else its tooltips one per line (placeholders such as
        ``{ability.0}`` left in). None when it says nothing."""
        if self.description:
            return self.description
        localization = self.data.get("Localization")
        tooltips = localization.get("Tooltips") if isinstance(localization, dict) else None
        lines = []
        for tooltip in tooltips if isinstance(tooltips, list) else ():
            content = tooltip.get("Content") if isinstance(tooltip, dict) else None
            line = _text(content.get("Text")) if isinstance(content, dict) else None
            if line:
                lines.append(line)
        return "\n".join(lines) or None

    @property
    def tags(self) -> tuple[str, ...]:
        """Its ``Tags`` then its ``HiddenTags`` (``Food``, ``Weapon``, ``Haste``, ``HeatedReference``, …)."""
        found: list[str] = []
        for key in ("Tags", "HiddenTags"):
            values = self.data.get(key)
            for value in values if isinstance(values, list) else ():
                if isinstance(value, str) and value not in found:
                    found.append(value)
        return tuple(found)

    @property
    def attributes(self) -> dict[str, object]:
        """The attributes of its starting tier (else its first tier), such as ``DamageAmount``: 10."""
        tiers = self.data.get("Tiers")
        if not isinstance(tiers, dict) or not tiers:
            return {}
        tier = tiers.get(self.starting_tier) if self.starting_tier in tiers else next(iter(tiers.values()))
        attributes = tier.get("Attributes") if isinstance(tier, dict) else None
        return dict(attributes) if isinstance(attributes, dict) else {}

    @property
    def mechanics(self) -> frozenset[str]:
        """Its tags and the attribute keys of every tier, less the ones every item has: what two cards
        share when they work on the same thing (``Haste``, ``HasteAmount``, ``HeatedReference``)."""
        keys = set(self.tags)
        tiers = self.data.get("Tiers")
        for tier in tiers.values() if isinstance(tiers, dict) else ():
            attributes = tier.get("Attributes") if isinstance(tier, dict) else None
            if isinstance(attributes, dict):
                keys.update(key for key in attributes if isinstance(key, str))
        return frozenset(keys - GENERIC_KEYS)


def game_folder() -> Path | None:
    """The game's data folder: `LocalLow\\Tempo Storm\\The Bazaar` on Windows, its Application Support
    folder on macOS. None without a home folder."""
    home = os.environ.get("USERPROFILE") if sys.platform == "win32" else os.environ.get("HOME")
    if not home:
        return None
    if sys.platform == "darwin":
        return Path(home) / "Library" / "Application Support" / "com.TempoStorm.TheBazaar"
    return Path(home) / "AppData" / "LocalLow" / "Tempo Storm" / "The Bazaar"


def candidates(root: Path) -> list[Path]:
    """Where the game may keep `GameData.db`: the tracker's four places, plus the PTR's cache."""
    name = "GameData.db"
    return [
        root / "prod" / "cache" / name,
        root / "ptr" / "cache" / name,
        root / "cache" / name,
        root / "prod" / name,
        root / name,
    ]


def newest_existing(paths: Iterable[Path]) -> Path | None:
    """The file among ``paths`` written most recently."""
    found = [(path.stat().st_mtime, path) for path in paths if path.is_file()]
    return max(found, key=lambda item: item[0])[1] if found else None


class CardCatalog:
    """Cards by id (ignoring case) and by name (ignoring case)."""

    def __init__(self, cards: Iterable[CardInfo] = (), source: Path | None = None) -> None:
        self._by_id: dict[str, CardInfo] = {}
        self._by_name: dict[str, list[CardInfo]] = {}
        self.source = source
        for card in cards:
            self._by_id[card.id.lower()] = card
        for card in self._by_id.values():
            self._by_name.setdefault(card.name.lower(), []).append(card)

    @classmethod
    def load(cls, root: Path | None = None) -> CardCatalog:
        """The catalog from the newest `GameData.db` under ``root`` (default: the game's folder),
        or an empty one when there is none or it can't be read."""
        root = root if root is not None else game_folder()
        path = newest_existing(candidates(root)) if root is not None else None
        if path is None:
            return cls()
        try:
            return cls.from_game_data_db(path)
        except (OSError, sqlite3.Error):
            return cls()

    @classmethod
    def from_game_data_db(cls, path: Path) -> CardCatalog:
        """The cards in the `GameData.db` at ``path``, read from a temporary copy."""
        handle, copy_name = tempfile.mkstemp(prefix=f"bazaar_advisor_gamedata_{os.getpid()}_", suffix=".db")
        os.close(handle)
        copy = Path(copy_name)
        try:
            shutil.copyfile(path, copy)
            connection = sqlite3.connect(f"{copy.resolve().as_uri()}?mode=ro", uri=True)
            try:
                rows = connection.execute("SELECT Id, Data FROM cards").fetchall()
            finally:
                connection.close()
        finally:
            copy.unlink(missing_ok=True)
        return cls(_parse_rows(rows), source=path)

    def get(self, card_id: str) -> CardInfo | None:
        """The card with this id, ignoring case."""
        return self._by_id.get(card_id.lower())

    def by_name(self, name: str) -> list[CardInfo]:
        """The cards with exactly this name, ignoring case and surrounding spaces."""
        return list(self._by_name.get(name.strip().lower(), ()))

    def search(self, query: str) -> list[CardInfo]:
        """Cards for ``query``, best first: the card with that id, else those with exactly that name,
        else those whose name has it in it, else those whose name has every word of it (ignoring case)."""
        query = query.strip().lower()
        if not query:
            return []
        if (card := self._by_id.get(query)) is not None:
            return [card]
        if exact := self._by_name.get(query):
            return list(exact)
        words = query.split()
        part = [card for card in self._by_id.values() if query in card.name.lower()]
        found = part or [card for card in self._by_id.values() if all(w in card.name.lower() for w in words)]
        return sorted(found, key=lambda card: (len(card.name), card.name.lower(), card.id))

    def __iter__(self) -> Iterator[CardInfo]:
        return iter(self._by_id.values())

    def __len__(self) -> int:
        return len(self._by_id)


def _parse_rows(rows: Iterable[tuple[object, object]]) -> Iterator[CardInfo]:
    for card_id, data in rows:
        if not isinstance(card_id, str) or not isinstance(data, (bytes, str)):
            continue
        try:
            card = json.loads(data)
        except (ValueError, UnicodeDecodeError):
            continue
        if isinstance(card, dict):
            yield parse_card(card_id, card)


def parse_card(card_id: str, card: dict[str, object]) -> CardInfo:
    """A card from its JSON, as the tracker reads it."""

    def localised(part: str) -> str | None:
        localization = card.get("Localization")
        entry = localization.get(part) if isinstance(localization, dict) else None
        return _text(entry.get("Text")) if isinstance(entry, dict) else None

    heroes = card.get("Heroes")
    return CardInfo(
        id=card_id,
        name=localised("Title") or _text(card.get("InternalName")) or card_id,
        starting_tier=_text(card.get("StartingTier")),
        size=_text(card.get("Size")),
        card_type=_text(card.get("Type")),
        description=localised("Description"),
        art_key=_text(card.get("ArtKey")) or None,
        heroes=tuple(h for h in map(_text, heroes) if h) if isinstance(heroes, list) else (),
        data=card,
    )


def _text(value: object) -> str | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, str):
        return value
    if isinstance(value, (int, float)):
        return str(value)
    return None
