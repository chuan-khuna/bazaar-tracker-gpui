"""Fixtures: a history.db built by the tracker's own migrations, and a small GameData.db."""

from __future__ import annotations

import json
import re
import sqlite3
from pathlib import Path

import pytest

SCHEMA_RS = Path(__file__).resolve().parents[3] / "crates" / "bazaar-tracker" / "src" / "history" / "schema.rs"


def tracker_migrations() -> list[str]:
    """The SQL of each migration in the tracker's schema.rs, in order."""
    text = SCHEMA_RS.read_text(encoding="utf-8")
    body = text.split("pub const MIGRATIONS: &[&str] = &[", 1)[1].split("\n];", 1)[0]
    body = "\n".join(line for line in body.splitlines() if not line.lstrip().startswith("//"))
    return re.findall(r'"((?:[^"\\]|\\.)*)"', body, flags=re.DOTALL)


def make_history_db(path: Path, version: int) -> Path:
    """A history.db at ``path`` brought to ``version`` by the tracker's migrations, as the tracker
    does: one migration after another, then ``user_version``."""
    connection = sqlite3.connect(path)
    try:
        connection.execute("PRAGMA journal_mode = WAL")
        for sql in tracker_migrations()[:version]:
            connection.executescript(sql)
        connection.execute(f"PRAGMA user_version = {version}")
        connection.commit()
    finally:
        connection.close()
    return path


@pytest.fixture
def history_db(tmp_path: Path) -> Path:
    """A history.db at version 9 with one tracked Run, ``run-1``."""
    path = make_history_db(tmp_path / "history.db", 9)
    connection = sqlite3.connect(path)
    connection.execute("INSERT INTO run_tracker_runs (id, started_ms) VALUES ('run-1', 0)")
    connection.commit()
    connection.close()
    return path


GAME_CARDS: dict[str, dict[str, object]] = {
    "TPL-PASTA": {
        "Type": "Item",
        "Size": "Small",
        "StartingTier": "Bronze",
        "Heroes": ["Common"],
        "InternalName": "Pasta",
        "Localization": {"Title": {"Text": "Pasta"}, "Description": {"Text": "Heal 10."}},
        "ArtKey": "pasta_art",
    },
    "TPL-CUTLASS": {
        "Type": "Item",
        "Size": "Medium",
        "StartingTier": "Silver",
        "Heroes": ["Vanessa"],
        "InternalName": "Cutlass_Internal",
        "Localization": {"Title": {"Text": "Cutlass"}},
        "ArtKey": "",
    },
    # No title: named by its internal name.
    "TPL-UNTITLED": {"Type": "Skill", "InternalName": "Hidden Skill"},
    # Another card with Cutlass's name.
    "TPL-CUTLASS-2": {"Type": "Item", "Localization": {"Title": {"Text": "cutlass"}}},
}


def make_game_data_db(path: Path, cards: dict[str, dict[str, object]] = GAME_CARDS) -> Path:
    """A GameData.db with a ``cards (Id, Data)`` table, the JSON stored as a blob as the game does,
    plus one row that isn't JSON."""
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.execute("CREATE TABLE cards (Id TEXT PRIMARY KEY, Data BLOB)")
    connection.executemany(
        "INSERT INTO cards VALUES (?, ?)",
        [(card_id, json.dumps(card).encode()) for card_id, card in cards.items()],
    )
    connection.execute("INSERT INTO cards VALUES ('TPL-BROKEN', ?)", (b"{not json",))
    connection.commit()
    connection.close()
    return path
