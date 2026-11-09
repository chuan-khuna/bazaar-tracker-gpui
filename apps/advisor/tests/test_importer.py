"""The importer: Scraped run files into the imported_* tables, replaced on re-import, card ids checked."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from conftest import make_game_data_db
from scraped_runs import (
    BALL,
    CARDS,
    CHEESE,
    CUTTER,
    DECANTER,
    LOLLIPOP,
    run_document,
    selection,
    write_record_example,
    write_run,
)
from sqlalchemy import Engine

from backend.db import open_history
from importer import import_builds
from importer.cli import main
from importer.scraped import ScrapedRunError, parse
from knowledge.cards import CardCatalog

TABLES = [
    "imported_runs",
    "imported_run_cards",
    "imported_picks",
    "imported_combats",
    "imported_pair_stats",
    "imported_pick_stats",
    "imported_adjacency_stats",
]


@pytest.fixture
def engine(history_db: Path) -> Engine:
    engine = open_history(history_db)
    yield engine
    engine.dispose()


def dump(engine: Engine) -> dict[str, list[tuple]]:
    """Every imported_* row (rowid left out), sorted."""
    with engine.connect() as connection:
        return {
            table: sorted(connection.exec_driver_sql(f"SELECT * FROM {table}").fetchall(), key=repr) for table in TABLES
        }


def rows(engine: Engine, sql: str) -> list[tuple]:
    with engine.connect() as connection:
        return [tuple(row) for row in connection.exec_driver_sql(sql).fetchall()]


def test_parse_reads_the_run_cards_picks_and_combats() -> None:
    document = run_document(
        "run-a",
        board_ids=(LOLLIPOP, CUTTER),
        skill_ids=("TPL-SKILL",),
        events=[
            selection(2, 3, [BALL, DECANTER, CHEESE], DECANTER),
            # Taken but not among the options: still a pick.
            selection(2, 4, [CHEESE], BALL),
            # The same card offered twice: two offers, one taken.
            selection(3, 1, [CUTTER, CUTTER], CUTTER),
            {"kind": "sale", "day": 3, "hour": 2},
        ],
        wins=7,
        losses=3,
        combats=[{"d": 1, "h": 3, "o": "Win", "p": False}, {"d": 2, "h": 6, "o": None, "p": True}],
    )
    document["summary"]["board"][1]["enchantmentOverride"] = "Fiery"
    document["summary"]["board"][1]["tierOverride"] = "Gold"
    scraped = parse(document)

    assert scraped.run == {
        "run_id": "run-a",
        "profile_id": "profile-1",
        "hero": "Jules",
        "season": 20,
        "created_at": document["created_at"],
        "wins": 7,
        "losses": 3,
        "victory_tier": None,
        "url": "https://bazaardb.gg/run/tracker/run-a",
    }
    assert [(c["card_id"], c["kind"], c["tier"], c["slot"], c["size"], c["enchantment"]) for c in scraped.cards] == [
        (LOLLIPOP, "board", "Bronze", 0, 3, None),
        (CUTTER, "board", "Gold", 3, 1, "Fiery"),
        ("TPL-SKILL", "skill", "Bronze", None, 2, None),
    ]
    assert [(p["day"], p["hour"], p["card_id"], p["taken"]) for p in scraped.picks] == [
        (2, 3, BALL, False),
        (2, 3, DECANTER, True),
        (2, 3, CHEESE, False),
        (2, 4, CHEESE, False),
        (2, 4, BALL, True),
        (3, 1, CUTTER, True),
        (3, 1, CUTTER, False),
    ]
    assert [(c["day"], c["hour"], c["outcome"]) for c in scraped.combats] == [(1, 3, "Win"), (2, 6, None)]
    assert scraped.titles[BALL] == "Laurel's Bouncy Ball"


@pytest.mark.parametrize(
    ("document", "why"),
    [
        ([], "not a JSON object"),
        ({"format": 2, "run_id": "x", "hero": "Jules"}, "format 2"),
        ({"format": 1, "hero": "Jules"}, "no run_id"),
        ({"format": 1, "run_id": "x"}, "no hero"),
    ],
)
def test_parse_refuses_what_it_cant_read(document: object, why: str) -> None:
    with pytest.raises(ScrapedRunError, match=why):
        parse(document)


def test_a_run_without_a_timeline_has_no_picks() -> None:
    document = run_document("run-a", board_ids=(BALL,))
    document["timeline"] = None
    assert parse(document).picks == []


def test_import_writes_every_table(tmp_path: Path, engine: Engine) -> None:
    run_ids = write_record_example(tmp_path)
    report = import_builds(engine, tmp_path)

    assert report.files == 15 and sorted(report.run_ids) == run_ids and not report.skipped
    assert rows(engine, "SELECT COUNT(*) FROM imported_runs WHERE hero = 'Jules' AND season = 20") == [(15,)]
    assert rows(engine, f"SELECT COUNT(DISTINCT run_id) FROM imported_run_cards WHERE card_id = '{BALL}'") == [(4,)]
    assert rows(engine, f"SELECT COUNT(*), SUM(taken) FROM imported_picks WHERE card_id = '{BALL}'") == [(15, 7)]
    assert rows(engine, "SELECT COUNT(*) FROM imported_combats") == [(15,)]
    assert report.stats[("Jules", 20)]["pair"] > 0


def test_importing_twice_changes_nothing(tmp_path: Path, engine: Engine) -> None:
    write_record_example(tmp_path)
    import_builds(engine, tmp_path)
    first = dump(engine)
    import_builds(engine, tmp_path)
    assert dump(engine) == first
    assert all(first[table] for table in ("imported_runs", "imported_run_cards", "imported_picks", "imported_pair_stats"))


def test_reimporting_a_run_replaces_its_rows(tmp_path: Path, engine: Engine) -> None:
    write_run(tmp_path, run_document("run-a", board_ids=(LOLLIPOP, CUTTER), events=[selection(1, 1, [BALL], BALL)]))
    import_builds(engine, tmp_path)
    write_run(tmp_path, run_document("run-a", board_ids=(DECANTER,), wins=3, losses=10, events=[]))
    import_builds(engine, tmp_path)

    assert rows(engine, "SELECT run_id, wins, losses FROM imported_runs") == [("run-a", 3, 10)]
    assert rows(engine, "SELECT card_id FROM imported_run_cards") == [(DECANTER,)]
    assert rows(engine, "SELECT COUNT(*) FROM imported_picks") == [(0,)]


def test_a_bad_file_is_skipped_and_reported(tmp_path: Path, engine: Engine) -> None:
    write_run(tmp_path, run_document("run-a", board_ids=(BALL,)))
    bad = tmp_path / "profile-1" / "runs" / "season-20" / "broken.json"
    bad.write_text("{not json", encoding="utf-8")
    newer = tmp_path / "profile-1" / "runs" / "season-20" / "newer.json"
    newer.write_text(json.dumps({"format": 2, "run_id": "x", "hero": "Jules"}), encoding="utf-8")

    report = import_builds(engine, tmp_path)
    assert report.run_ids == ["run-a"]
    assert {path.name: why.split(":")[0] for path, why in report.skipped} == {"broken.json": "not readable JSON", "newer.json": "format 2, not 1"}


def test_a_card_id_not_in_the_catalog_is_reported_and_still_imported(tmp_path: Path, engine: Engine) -> None:
    known = {card_id: {"Type": "Item", "Localization": {"Title": {"Text": title}}} for card_id, (title, _) in CARDS.items()}
    del known[CHEESE]
    catalog = CardCatalog.from_game_data_db(make_game_data_db(tmp_path / "game" / "GameData.db", known))
    builds = tmp_path / "builds"
    write_run(builds, run_document("run-a", board_ids=(BALL, CHEESE)))
    write_run(builds, run_document("run-b", board_ids=(BALL,), events=[selection(1, 2, [CHEESE, BALL], BALL)]))

    report = import_builds(engine, builds, catalog)
    assert report.catalog_checked
    assert set(report.unknown_cards) == {CHEESE}
    unknown = report.unknown_cards[CHEESE]
    assert unknown.title == "Cheese Wheel" and unknown.run_ids == {"run-a", "run-b"}
    assert rows(engine, f"SELECT run_id FROM imported_run_cards WHERE card_id = '{CHEESE}'") == [("run-a",)]
    assert rows(engine, f"SELECT run_id FROM imported_picks WHERE card_id = '{CHEESE}'") == [("run-b",)]


def test_with_no_catalog_card_ids_are_not_checked(tmp_path: Path, engine: Engine) -> None:
    write_run(tmp_path, run_document("run-a", board_ids=(BALL,)))
    report = import_builds(engine, tmp_path, CardCatalog())
    assert not report.catalog_checked and not report.unknown_cards


def test_runs_with_no_season_are_imported_but_get_no_stats(tmp_path: Path, engine: Engine) -> None:
    for i in range(3):
        write_run(tmp_path, run_document(f"old-{i}", season=None, board_ids=(BALL, CUTTER)))
    report = import_builds(engine, tmp_path)
    assert rows(engine, "SELECT COUNT(*) FROM imported_runs WHERE season IS NULL") == [(3,)]
    assert report.stats == {}
    assert rows(engine, "SELECT COUNT(*) FROM imported_pair_stats") == [(0,)]


def test_the_cli_imports_and_lists_unknown_cards(tmp_path: Path, history_db: Path, capsys: pytest.CaptureFixture[str]) -> None:
    builds = tmp_path / "builds"
    write_run(builds, run_document("run-a", board_ids=(BALL, "TPL-NEW")))
    game = tmp_path / "game"
    make_game_data_db(game / "GameData.db", {BALL: {"Type": "Item"}})

    assert main(["--db", str(history_db), "--builds", str(builds), "--game-dir", str(game)]) == 0
    out = capsys.readouterr().out
    assert "Imported 1 runs from 1 files" in out
    assert "Jules S20: 0 pairs" in out
    assert "1 card ids not in" in out and "TPL-NEW" in out


def test_the_cli_stops_on_a_missing_builds_folder_or_old_history_db(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    from conftest import make_history_db

    assert main(["--db", str(tmp_path / "history.db"), "--builds", str(tmp_path / "nothing")]) == 1
    assert "No Scraped runs" in capsys.readouterr().err

    write_run(tmp_path / "builds", run_document("run-a"))
    old = make_history_db(tmp_path / "history.db", 8)
    assert main(["--db", str(old), "--builds", str(tmp_path / "builds")]) == 1
    assert "schema version 8" in capsys.readouterr().err
