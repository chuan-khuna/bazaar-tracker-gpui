"""Where the Advisor's tools get their numbers: history.db (the imported Scraped runs) and the card
catalog (`GameData.db`). Both are opened the first time a tool needs them and kept for the process.

A history.db that can't be used (missing, or at another schema version) is a ``ValueError`` from
``connection()``, so the tool fails with a message the model can read and pass on.
"""

from __future__ import annotations

from collections.abc import Iterator
from contextlib import contextmanager
from functools import cache
from pathlib import Path

from sqlalchemy import Connection, Engine

import config
from backend.db import HistoryDbError, open_history
from knowledge.cards import CardCatalog


class Sources:
    def __init__(
        self,
        history_db: Path | None = None,
        *,
        engine: Engine | None = None,
        catalog: CardCatalog | None = None,
        game_dir: Path | None = None,
    ) -> None:
        self.history_db = history_db
        self.game_dir = game_dir
        self._engine = engine
        self._catalog = catalog

    @classmethod
    def from_config(cls, settings: config.Config) -> Sources:
        return cls(settings.history_db)

    @contextmanager
    def connection(self) -> Iterator[Connection]:
        """A connection to history.db; ``ValueError`` when it can't be used."""
        if self._engine is None:
            if self.history_db is None:
                raise ValueError("No history.db is set, so there are no Scraped runs to count.")
            try:
                self._engine = open_history(self.history_db)
            except HistoryDbError as error:
                raise ValueError(str(error)) from None
        with self._engine.connect() as connection:
            yield connection

    @property
    def catalog(self) -> CardCatalog:
        """The card catalog, loaded on first use (empty when no `GameData.db` was found)."""
        if self._catalog is None:
            self._catalog = CardCatalog.load(self.game_dir)
        return self._catalog


@cache
def default_sources() -> Sources:
    """The process's sources, from the config: the data folder's history.db and the game's catalog."""
    return Sources.from_config(config.load())
