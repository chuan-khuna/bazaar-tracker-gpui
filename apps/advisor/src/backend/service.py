"""The service's parts: history.db, opened on first use, and the ``Store`` and ``Runs`` built on it.

The service starts whatever state history.db is in, so `GET /api/health` can say what is wrong. The
file is opened (and its version checked, ``open_history``) the first time a route needs it, and again
on every later request until it opens: after starting the tracker once to migrate an older file, the
Advisor works without a restart. Until then those routes answer 503 with the reason.
"""

from __future__ import annotations

import logging
import sqlite3
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from sqlalchemy import Engine

from agents.advisor.sources import Sources
from backend.agent.runner import ProviderFor, Runs
from backend.db import SCHEMA_VERSION, HistoryDbError, open_history
from backend.db.store import Store
from knowledge.cards import CardCatalog

log = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class Ready:
    store: Store
    runs: Runs


class Service:
    def __init__(
        self,
        history_db: Path,
        provider_for: ProviderFor,
        model: str,
        *,
        catalog: CardCatalog | None = None,
        game_dir: Path | None = None,
    ) -> None:
        self.history_db = history_db
        self._provider_for = provider_for
        self._model = model
        self._catalog = catalog
        self._game_dir = game_dir
        self._engine: Engine | None = None
        self._ready: Ready | None = None

    def ready(self) -> Ready:
        """The Store and Runs; ``HistoryDbError`` while history.db can't be used."""
        if self._ready is None:
            engine = open_history(self.history_db)
            store = Store(engine)
            sources = Sources(engine=engine, catalog=self._catalog, game_dir=self._game_dir)
            self._engine = engine
            self._ready = Ready(store, Runs(store, sources, self._provider_for, self._model))
            log.info("Opened %s (schema version %d)", self.history_db, SCHEMA_VERSION)
        return self._ready

    def history_status(self) -> dict[str, Any]:
        """history.db's path, its ``user_version`` (None when it can't be read), the version the
        Advisor needs, and why it can't be used (None when it can)."""
        version: int | None = None
        error: str | None = None
        path = self.history_db.resolve()
        if not path.is_file():
            error = f"No history.db at {path}."
        else:
            try:
                connection = sqlite3.connect(f"{path.as_uri()}?mode=ro", uri=True)
                try:
                    version = connection.execute("PRAGMA user_version").fetchone()[0]
                finally:
                    connection.close()
            except sqlite3.Error as failure:
                error = f"Can't read {path}: {failure}"
        if error is None:
            try:
                self.ready()
            except HistoryDbError as failure:
                error = str(failure)
        return {"path": str(path), "version": version, "expected": SCHEMA_VERSION, "error": error}

    def close(self) -> None:
        if self._engine is not None:
            self._engine.dispose()
