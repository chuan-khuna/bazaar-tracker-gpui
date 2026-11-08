"""Opening history.db: the version check, the pragmas, and the models against the real schema."""

from __future__ import annotations

import sqlite3
from pathlib import Path

import pytest
from conftest import make_history_db, tracker_migrations
from sqlalchemy import inspect, select
from sqlalchemy.exc import IntegrityError

from backend.db import SCHEMA_VERSION, HistoryDbError, SchemaVersionError, open_history, sessions
from backend.db.models import AdvisorConversation, AdvisorMessage, Base, ImportedPick, ImportedRun


def test_schema_version_is_the_tracker_latest() -> None:
    assert len(tracker_migrations()) == SCHEMA_VERSION


def test_opens_a_file_at_the_expected_version_with_the_pragmas(history_db: Path) -> None:
    engine = open_history(history_db)
    with engine.connect() as connection:
        assert connection.exec_driver_sql("PRAGMA journal_mode").scalar_one() == "wal"
        assert connection.exec_driver_sql("PRAGMA foreign_keys").scalar_one() == 1
        assert connection.exec_driver_sql("PRAGMA busy_timeout").scalar_one() == 5000
    engine.dispose()


@pytest.mark.parametrize(("version", "hint"), [(8, "just tracker run"), (10, "update apps/advisor")])
def test_refuses_another_version_naming_both(tmp_path: Path, version: int, hint: str) -> None:
    path = make_history_db(tmp_path / "history.db", min(version, SCHEMA_VERSION))
    connection = sqlite3.connect(path)
    connection.execute(f"PRAGMA user_version = {version}")
    connection.close()

    with pytest.raises(SchemaVersionError) as caught:
        open_history(path)

    message = str(caught.value)
    assert f"schema version {version}," in message
    assert f"written for version {SCHEMA_VERSION}." in message
    assert hint in message
    assert caught.value.found == version


def test_an_older_file_is_left_untouched(tmp_path: Path) -> None:
    path = make_history_db(tmp_path / "history.db", 8)
    with pytest.raises(SchemaVersionError):
        open_history(path)

    connection = sqlite3.connect(path)
    assert connection.execute("PRAGMA user_version").fetchone()[0] == 8
    tables = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type = 'table'")}
    connection.close()
    assert "advisor_conversations" not in tables


def test_a_missing_file_is_refused_and_not_created(tmp_path: Path) -> None:
    path = tmp_path / "history.db"
    with pytest.raises(HistoryDbError, match="No history.db"):
        open_history(path)
    assert not path.exists()


def test_models_match_the_tables(history_db: Path) -> None:
    """Every model's columns are its table's, in the file the tracker's migrations make."""
    engine = open_history(history_db)
    inspector = inspect(engine)
    for table in Base.metadata.sorted_tables:
        in_file = {column["name"]: column["nullable"] for column in inspector.get_columns(table.name)}
        in_model = {column.name: column.nullable for column in table.columns if column.name != "rowid"}
        if set(in_model) != set(in_file):
            pytest.fail(f"{table.name}: model {sorted(in_model)} vs file {sorted(in_file)}")
        for name, nullable in in_model.items():
            # A primary key column reads as nullable in SQLite's table_info unless NOT NULL is spelt out.
            if not table.columns[name].primary_key:
                assert nullable == in_file[name], f"{table.name}.{name} nullable"
    engine.dispose()


def test_writes_and_cascades_through_the_models(history_db: Path) -> None:
    engine = open_history(history_db)
    with sessions(engine)() as session:
        conversation = AdvisorConversation(id="c1", run_id="run-1", title="Which?", day=5, hour=2, created_ms=1)
        conversation.messages.append(
            AdvisorMessage(seq=0, role="user", message={"role": "user", "content": "Which?"}, created_ms=1)
        )
        conversation.messages.append(
            AdvisorMessage(seq=1, role="assistant", message={"role": "assistant"}, stats={"toolCalls": 0}, created_ms=2)
        )
        run = ImportedRun(run_id="r1", hero="Vanessa", season=3)
        run.picks.append(ImportedPick(day=1, hour=1, card_id="TPL-PASTA", taken=True))
        session.add_all([conversation, run])
        session.commit()

    raw = sqlite3.connect(history_db)
    assert raw.execute("SELECT message, stats FROM advisor_messages WHERE seq = 0").fetchone() == (
        '{"role": "user", "content": "Which?"}',
        None,
    )
    assert raw.execute("SELECT stats FROM advisor_messages WHERE seq = 1").fetchone() == ('{"toolCalls": 0}',)
    assert raw.execute("SELECT taken FROM imported_picks").fetchone() == (1,)
    raw.close()

    with sessions(engine)() as session:
        # Foreign keys are on: a Conversation must belong to a Run the tracker knows.
        session.add(AdvisorConversation(id="c2", run_id="no-such-run", title="x", created_ms=1))
        with pytest.raises(IntegrityError, match="FOREIGN KEY"):
            session.commit()
        session.rollback()

        # Deleting a Conversation deletes its messages, by the schema's cascade.
        session.delete(session.get(AdvisorConversation, "c1"))
        session.commit()
        assert session.scalars(select(AdvisorMessage)).all() == []
    engine.dispose()
