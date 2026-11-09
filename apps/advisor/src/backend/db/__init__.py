"""history.db: opening it safely, and SQLAlchemy models for the tables the Advisor uses.

The tracker owns the schema (crates/bazaar-tracker/src/history/schema.rs). The Advisor never
migrates: it opens a file at exactly ``SCHEMA_VERSION`` or refuses.
"""

from backend.db.connection import SCHEMA_VERSION, HistoryDbError, SchemaVersionError, open_history, sessions

__all__ = ["SCHEMA_VERSION", "HistoryDbError", "SchemaVersionError", "open_history", "sessions"]
