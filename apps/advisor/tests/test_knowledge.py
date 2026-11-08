"""knowledge/: the formulas, the season sample, and the pair and pick queries, against the record's
example numbers and small hand-computed cases."""

from __future__ import annotations

from pathlib import Path

import pytest
from scraped_runs import (
    BALL,
    CHEESE,
    CUTTER,
    DECANTER,
    LOLLIPOP,
    record_run_ids,
    run_document,
    write_record_example,
    write_run,
)
from sqlalchemy import Connection

import knowledge
from backend.db import open_history
from importer import import_builds
from knowledge.stats import BoardCard, RunData, adjacency_stats, pair_stats, pick_stats, size_slots


@pytest.fixture
def record(tmp_path: Path, history_db: Path) -> Connection:
    """history.db with the record's 15 Jules runs imported."""
    write_record_example(tmp_path / "builds")
    engine = open_history(history_db)
    import_builds(engine, tmp_path / "builds")
    with engine.connect() as connection:
        yield connection
    engine.dispose()


def imported(tmp_path: Path, history_db: Path, documents: list[dict], min_support: int = 3) -> Connection:
    for document in documents:
        write_run(tmp_path / "builds", document)
    engine = open_history(history_db)
    import_builds(engine, tmp_path / "builds", min_support=min_support)
    return engine


# --- the record's example ---------------------------------------------------------------------


def test_the_record_example_lifts(record: Connection) -> None:
    ball_lollipop = knowledge.together(record, "Jules", BALL, LOLLIPOP)
    ball_cutter = knowledge.together(record, "Jules", BALL, CUTTER)
    assert (ball_lollipop.pair.n, round(ball_lollipop.pair.lift, 2)) == (3, 2.25)
    assert (ball_cutter.pair.n, round(ball_cutter.pair.lift, 2)) == (3, 2.81)
    assert ball_cutter.pair.run_ids == ("jules-01", "jules-02", "jules-04")
    assert ball_cutter.sample.label == "S20 (15 runs)"

    decanter_cutter = knowledge.together(record, "Jules", DECANTER, CUTTER).pair
    assert (decanter_cutter.n, decanter_cutter.lift) == (3, 2.25)
    assert knowledge.together(record, "Jules", DECANTER, LOLLIPOP).pair.n == 2
    assert knowledge.together(record, "Jules", CHEESE, LOLLIPOP).pair.n == 1
    assert knowledge.together(record, "Jules", CHEESE, CUTTER).pair.n == 2


def test_the_record_example_runs_with_each_option(record: Connection) -> None:
    counts = {card: knowledge.card_runs(record, "Jules", card) for card in (BALL, DECANTER, CHEESE)}
    assert {card: (found.n, found.sample.n) for card, found in counts.items()} == {
        BALL: (4, 15),
        DECANTER: (5, 15),
        CHEESE: (4, 15),
    }
    assert counts[BALL].run_ids == ("jules-01", "jules-02", "jules-03", "jules-04")


def test_the_record_example_picks(record: Connection) -> None:
    found = {card: knowledge.picks(record, "Jules", card) for card in (BALL, DECANTER, CHEESE)}
    assert {card: (p.offered, p.taken) for card, p in found.items()} == {BALL: (15, 7), DECANTER: (16, 9), CHEESE: (15, 11)}
    assert len(found[BALL].run_ids) == 15 and found[BALL].taken_run_ids == tuple(record_run_ids()[:7])
    assert found[BALL].sample.seasons == (knowledge.SeasonCount(20, 15),)


def test_the_record_example_in_the_stats_table(record: Connection) -> None:
    stored = {
        (a, b): (n, round(lift, 2))
        for a, b, n, lift in record.exec_driver_sql(
            "SELECT a, b, n, lift FROM imported_pair_stats WHERE hero = 'Jules' AND season = 20"
        )
    }
    assert stored[(BALL, LOLLIPOP)] == (3, 2.25)
    assert stored[(LOLLIPOP, BALL)] == (3, 2.25)
    assert stored[(BALL, CUTTER)] == (3, 2.81)
    assert stored[(DECANTER, CUTTER)] == (3, 2.25)
    # Under the minimum support of 3: dropped.
    assert (DECANTER, LOLLIPOP) not in stored and (CHEESE, CUTTER) not in stored


def test_partners_drop_pairs_under_the_minimum_support(record: Connection) -> None:
    found = knowledge.partners(record, "Jules", CUTTER)
    assert found.n == 4
    assert [(p.b, p.n, round(p.lift, 2)) for p in found.partners] == [(BALL, 3, 2.81), (DECANTER, 3, 2.25)]
    assert [p.b for p in knowledge.partners(record, "Jules", CUTTER, min_support=2).partners] == [
        BALL,
        DECANTER,
        CHEESE,
        LOLLIPOP,
    ]


def test_a_pair_never_together_has_n_zero(record: Connection) -> None:
    pair = knowledge.together(record, "Jules", BALL, "TPL-FILLER-0").pair
    assert (pair.n, pair.lift, pair.run_ids) == (1, 3.75, ("jules-01",))
    never = knowledge.together(record, "Jules", BALL, "TPL-NOWHERE").pair
    assert (never.n, never.lift, never.win_gain, never.run_ids) == (0, None, None, ())


# --- the formulas -----------------------------------------------------------------------------


def run(run_id: str, cards: tuple[str, ...] = (), wins: int = 0, losses: int = 0, taken: tuple[str, ...] = ()) -> RunData:
    return RunData(run_id, wins, losses, board=[BoardCard(c, None, None) for c in cards], taken=list(taken))


def test_win_rate_gain_with_the_beta_prior() -> None:
    runs = [
        run("r1", ("A", "B"), wins=10, losses=0),
        run("r2", ("A", "B"), wins=6, losses=4),
        run("r3", ("A",), wins=2, losses=8),
        run("r4", ("C",), wins=4, losses=4),
    ]
    stats = {(s.a, s.b): s for s in pair_stats(runs, min_support=1)}
    # The prior: 10 games at the sample's rate, 22 wins of 38 games: a = 220/38, b = 160/38.
    a = 220 / 38
    with_b = (16 + a) / (20 + 10)  # r1 + r2
    without_b = (2 + a) / (10 + 10)  # r3
    assert stats[("A", "B")].win_gain == pytest.approx(with_b - without_b)
    assert stats[("A", "B")].win_gain == pytest.approx(0.33684, abs=1e-5)
    # Every run with B has A: nothing to compare against.
    assert stats[("B", "A")].win_gain is None
    assert stats[("A", "B")].lift == pytest.approx(2 * 4 / (3 * 2))


def test_one_big_run_doesnt_top_the_list() -> None:
    runs = [run(f"r{i}", ("A", "B"), wins=5, losses=5) for i in range(10)]
    runs += [run(f"s{i}", ("A",), wins=5, losses=5) for i in range(10)]
    runs += [run("lucky", ("A", "X"), wins=10, losses=0)]
    gains = {s.b: s.win_gain for s in pair_stats(runs, min_support=1) if s.a == "A"}
    # Unsmoothed, X would gain 0.5 (10 of 10 against 100 of 200); the prior about halves it.
    assert 0 < gains["X"] < 0.3


def test_size_slots() -> None:
    assert [size_slots(s) for s in ("Small", "medium", "LARGE", None, "Huge")] == [1, 2, 3, None, None]


def test_adjacency_from_slot_and_size() -> None:
    # A (Large, 0–2), B (Small, 3), gap at 4, C (Medium, 5–6), D (Small, 7).
    board = [BoardCard("A", 0, 3), BoardCard("B", 3, 1), BoardCard("C", 5, 2), BoardCard("D", 7, 1)]
    one = RunData("r1", board=board)
    assert one.adjacent_pairs() == {("A", "B"), ("B", "A"), ("C", "D"), ("D", "C")}
    two = RunData("r2", board=[BoardCard("A", 2, 3), BoardCard("C", 0, 2)])
    stats = {(s.a, s.b): s for s in adjacency_stats([one, two], min_support=1)}
    assert set(stats) == {("A", "B"), ("B", "A"), ("C", "D"), ("D", "C"), ("A", "C"), ("C", "A")}
    assert stats[("A", "C")].n == 1 and stats[("A", "C")].run_ids == ("r2",)
    # A and C are on both Boards and side by side on one: lift 1 × 2 / (2 × 2).
    assert stats[("A", "C")].lift == 0.5


def test_adjacency_in_the_stats_table(record: Connection) -> None:
    # Giant Lollipop sits right before Pizza Cutter in two runs: under the support of 3, so no row.
    assert not record.exec_driver_sql("SELECT * FROM imported_adjacency_stats WHERE a = ? AND b = ?", (LOLLIPOP, CUTTER)).fetchall()


def test_adjacency_kept_with_a_lower_support(tmp_path: Path, history_db: Path) -> None:
    write_record_example(tmp_path / "builds")
    engine = open_history(history_db)
    import_builds(engine, tmp_path / "builds", min_support=2)
    with engine.connect() as connection:
        found = connection.exec_driver_sql(
            "SELECT n, lift FROM imported_adjacency_stats WHERE a = ? AND b = ?", (LOLLIPOP, CUTTER)
        ).fetchall()
    engine.dispose()
    assert [(n, round(lift, 2)) for n, lift in found] == [(2, 1.5)]


def test_pick_after_pick() -> None:
    runs = [
        run("r1", taken=("A", "B", "C")),
        run("r2", taken=("B", "A")),
        run("r3", taken=("A", "C", "A")),
    ]
    stats = {(s.a, s.b): s.n for s in pick_stats(runs, min_support=1)}
    assert stats == {("A", "B"): 1, ("A", "C"): 2, ("B", "C"): 1, ("B", "A"): 1, ("C", "A"): 1}


def test_picked_after_query(tmp_path: Path, history_db: Path) -> None:
    from scraped_runs import selection

    documents = [
        run_document(f"r{i}", events=[selection(1, 1, [BALL], BALL), selection(2, 1, [DECANTER, CHEESE], DECANTER)])
        for i in range(3)
    ]
    engine = imported(tmp_path, history_db, documents)
    with engine.connect() as connection:
        found = knowledge.picked_after(connection, "Jules", BALL)
        stored = connection.exec_driver_sql("SELECT a, b, n FROM imported_pick_stats").fetchall()
    engine.dispose()
    assert found.n == 3 and [(p.b, p.n, p.run_ids) for p in found.after] == [(DECANTER, 3, ("r0", "r1", "r2"))]
    assert stored == [(BALL, DECANTER, 3)]


# --- the season sample ------------------------------------------------------------------------


def test_a_full_current_season_is_used_alone(record: Connection) -> None:
    found = knowledge.sample(record, "jules")
    assert found.seasons == (knowledge.SeasonCount(20, 15),) and found.n == 15 and not found.thin
    assert found.label == "S20 (15 runs)"


def test_a_thin_season_adds_the_previous_one(tmp_path: Path, history_db: Path) -> None:
    documents = [run_document(f"jules-{i}", board_ids=(BALL,)) for i in range(10)]
    documents += [run_document(f"v20-{i}", hero="Vanessa", season=20) for i in range(3)]
    documents += [run_document(f"v19-{i}", hero="Vanessa", season=19) for i in range(20)]
    documents += [run_document(f"v18-{i}", hero="Vanessa", season=18) for i in range(5)]
    engine = imported(tmp_path, history_db, documents)
    with engine.connect() as connection:
        vanessa = knowledge.sample(connection, "Vanessa")
        jules = knowledge.sample(connection, "Jules")
        strict = knowledge.sample(connection, "Vanessa", min_runs=2)
        picks = knowledge.picks(connection, "Vanessa", BALL)
    engine.dispose()

    assert vanessa.label == "S20 (3 runs) + S19 (20 runs)"
    assert vanessa.n == 23 and not vanessa.thin
    assert {run_id[:3] for run_id in vanessa.run_ids} == {"v20", "v19"}
    assert jules.label == "S20 (10 runs)"
    assert strict.label == "S20 (3 runs)"
    assert picks.sample == vanessa


def test_the_current_season_is_the_newest_of_any_hero(tmp_path: Path, history_db: Path) -> None:
    documents = [run_document("jules-1", season=21)]
    documents += [run_document(f"v20-{i}", hero="Vanessa", season=20) for i in range(4)]
    documents += [run_document(f"v19-{i}", hero="Vanessa", season=19) for i in range(4)]
    engine = imported(tmp_path, history_db, documents)
    with engine.connect() as connection:
        found = knowledge.sample(connection, "Vanessa")
        older = knowledge.sample(connection, "Vanessa", season=20)
        assert knowledge.current_season(connection) == 21
    engine.dispose()
    assert found.label == "S21 (0 runs) + S20 (4 runs)" and found.thin
    assert older.label == "S20 (4 runs) + S19 (4 runs)" and older.thin


def test_no_runs_is_an_empty_sample(history_db: Path) -> None:
    engine = open_history(history_db)
    with engine.connect() as connection:
        found = knowledge.sample(connection, "Jules")
        partners = knowledge.partners(connection, "Jules", BALL)
    engine.dispose()
    assert (found.seasons, found.n, found.thin) == ((), 0, True)
    assert partners.n == 0 and partners.partners == ()
