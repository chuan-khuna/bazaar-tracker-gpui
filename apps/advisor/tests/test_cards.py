"""The GameData.db reader: the newest copy, lookups by id and by name, and the temporary copy."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

from conftest import make_game_data_db

from knowledge.cards import CardCatalog


def test_looks_cards_up_by_id_ignoring_case(tmp_path: Path) -> None:
    make_game_data_db(tmp_path / "prod" / "cache" / "GameData.db")
    catalog = CardCatalog.load(tmp_path)

    pasta = catalog.get("tpl-pasta")
    assert pasta is not None
    assert (pasta.id, pasta.name, pasta.size, pasta.starting_tier, pasta.card_type) == (
        "TPL-PASTA",
        "Pasta",
        "Small",
        "Bronze",
        "Item",
    )
    assert pasta.description == "Heal 10."
    assert pasta.heroes == ("Common",)
    assert pasta.art_key == "pasta_art"
    assert catalog.get("TPL-NOPE") is None


def test_looks_cards_up_by_name_ignoring_case(tmp_path: Path) -> None:
    make_game_data_db(tmp_path / "prod" / "cache" / "GameData.db")
    catalog = CardCatalog.load(tmp_path)

    assert {card.id for card in catalog.by_name(" CUTLASS ")} == {"TPL-CUTLASS", "TPL-CUTLASS-2"}
    assert [card.id for card in catalog.by_name("Hidden Skill")] == ["TPL-UNTITLED"]
    assert catalog.by_name("Nothing") == []


def test_reads_fields_as_the_tracker_does(tmp_path: Path) -> None:
    catalog = CardCatalog.from_game_data_db(make_game_data_db(tmp_path / "GameData.db"))

    cutlass = catalog.get("TPL-CUTLASS")
    assert cutlass is not None
    assert cutlass.art_key is None  # an empty ArtKey counts as none
    assert cutlass.description is None
    # The row that isn't JSON is skipped; the other four are read.
    assert len(catalog) == 4


def test_the_newest_file_wins(tmp_path: Path) -> None:
    old = make_game_data_db(tmp_path / "prod" / "cache" / "GameData.db", {"TPL-OLD": {"InternalName": "Old"}})
    new = make_game_data_db(tmp_path / "ptr" / "cache" / "GameData.db", {"TPL-NEW": {"InternalName": "New"}})
    os.utime(old, (1_000_000, 1_000_000))
    os.utime(new, (2_000_000, 2_000_000))

    catalog = CardCatalog.load(tmp_path)

    assert catalog.source == new
    assert catalog.get("TPL-NEW") is not None
    assert catalog.get("TPL-OLD") is None


def test_reads_from_a_copy_and_deletes_it(tmp_path: Path) -> None:
    def copies() -> set[str]:
        return {name for name in os.listdir(tempfile.gettempdir()) if name.startswith("bazaar_advisor_gamedata_")}

    before = copies()
    CardCatalog.from_game_data_db(make_game_data_db(tmp_path / "GameData.db"))
    assert copies() == before


def test_nothing_found_is_an_empty_catalog(tmp_path: Path) -> None:
    assert len(CardCatalog.load(tmp_path)) == 0
    (tmp_path / "GameData.db").write_bytes(b"not a database")
    assert len(CardCatalog.load(tmp_path)) == 0
