"""Opening history.db beside the running tracker, without ever changing its schema.

The tracker writes the same file, so every connection uses WAL (readers never block the writer),
``foreign_keys=ON`` (the cascades from a Run to its Conversations work) and a ``busy_timeout``
(a write waits for the tracker's instead of failing at once). The file must already exist and be
at ``SCHEMA_VERSION``: an older one is the tracker's to migrate, a newer one needs a newer Advisor.
"""

from __future__ import annotations

import sqlite3
from pathlib import Path

from sqlalchemy import Engine, create_engine, event
from sqlalchemy.orm import Session, sessionmaker

# The history.db schema version (PRAGMA user_version) the Advisor's models are written for:
# migration 9 in crates/bazaar-tracker/src/history/schema.rs.
SCHEMA_VERSION = 9

# How long a statement waits for another connection's lock (the tracker's) before failing.
BUSY_TIMEOUT_MS = 5_000


class HistoryDbError(RuntimeError):
    """history.db can't be used: missing, unreadable, or at the wrong version."""


class SchemaVersionError(HistoryDbError):
    """history.db is at another schema version than ``SCHEMA_VERSION``."""

    def __init__(self, path: Path, found: int) -> None:
        self.path = path
        self.found = found
        if found < SCHEMA_VERSION:
            hint = "Start the tracker (`just tracker run`) once to migrate it."
        else:
            hint = "The tracker is newer than the Advisor: update apps/advisor for the new schema."
        super().__init__(
            f"{path} is at schema version {found}, but the Advisor is written for version "
            f"{SCHEMA_VERSION}. {hint}"
        )


def open_history(path: Path) -> Engine:
    """An engine on the existing history.db at ``path``, checked to be at ``SCHEMA_VERSION``.

    Raises ``HistoryDbError`` (or its ``SchemaVersionError``) when the file can't be used. The
    file is never created: the tracker makes it.
    """
    path = path.resolve()
    if not path.is_file():
        raise HistoryDbError(
            f"No history.db at {path}. Start the tracker once (`just tracker run`) to create it, "
            "or set BAZAAR_TRACKER_DATA_DIR to the tracker's data folder."
        )
    # mode=rw: open the file read-write but never create it.
    uri = f"{path.as_uri()}?mode=rw"

    def connect() -> sqlite3.Connection:
        return sqlite3.connect(uri, uri=True, timeout=BUSY_TIMEOUT_MS / 1000, check_same_thread=False)

    engine = create_engine("sqlite://", creator=connect)
    event.listen(engine, "connect", _set_pragmas)
    try:
        with engine.connect() as connection:
            found = connection.exec_driver_sql("PRAGMA user_version").scalar_one()
    except Exception as error:
        engine.dispose()
        raise HistoryDbError(f"Can't open {path}: {error}") from error
    if found != SCHEMA_VERSION:
        engine.dispose()
        raise SchemaVersionError(path, found)
    return engine


def sessions(engine: Engine) -> sessionmaker[Session]:
    """A session factory on ``engine``."""
    return sessionmaker(engine, expire_on_commit=False)


def _set_pragmas(dbapi_connection: sqlite3.Connection, _record: object) -> None:
    cursor = dbapi_connection.cursor()
    try:
        cursor.execute(f"PRAGMA busy_timeout = {BUSY_TIMEOUT_MS}")
        cursor.execute("PRAGMA journal_mode = WAL")
        cursor.execute("PRAGMA foreign_keys = ON")
    finally:
        cursor.close()
