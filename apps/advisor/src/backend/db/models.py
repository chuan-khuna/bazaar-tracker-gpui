"""SQLAlchemy models for the advisor_* and imported_* tables in history.db (schema version 9).

They describe tables the tracker made (migration 9 in crates/bazaar-tracker/src/history/schema.rs);
nothing here creates or alters a table. ``tests/test_models.py`` checks them against the real
migrations. Keep them in step when the tracker adds a migration that touches these tables.

- ``message`` and ``stats`` are JSON stored as TEXT.
- ``imported_run_cards``, ``imported_picks`` and ``imported_combats`` have no primary key, so
  their models map SQLite's ``rowid``.
- ``advisor_conversations.run_id`` references ``run_tracker_runs(id)``, the tracker's table, which
  has no model here: SQLite enforces it, and the cascades move or delete a Run's Conversations.
"""

from __future__ import annotations

from sqlalchemy import JSON, Boolean, ForeignKey, Integer, Text
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship


class Base(DeclarativeBase):
    pass


class AdvisorConversation(Base):
    __tablename__ = "advisor_conversations"

    id: Mapped[str] = mapped_column(Text, primary_key=True)
    run_id: Mapped[str] = mapped_column(Text)
    title: Mapped[str] = mapped_column(Text)
    day: Mapped[int | None] = mapped_column(Integer)
    hour: Mapped[int | None] = mapped_column(Integer)
    created_ms: Mapped[int] = mapped_column(Integer)

    messages: Mapped[list[AdvisorMessage]] = relationship(
        back_populates="conversation", order_by="AdvisorMessage.seq", passive_deletes=True
    )


class AdvisorMessage(Base):
    __tablename__ = "advisor_messages"

    conversation_id: Mapped[str] = mapped_column(
        Text, ForeignKey("advisor_conversations.id", onupdate="CASCADE", ondelete="CASCADE"), primary_key=True
    )
    seq: Mapped[int] = mapped_column(Integer, primary_key=True)
    role: Mapped[str] = mapped_column(Text)
    # A tau_agent message as JSON.
    message: Mapped[object] = mapped_column(JSON)
    # The reply's usage (model, time, tokens, cost, tool calls), on its last message only.
    stats: Mapped[object | None] = mapped_column(JSON(none_as_null=True))
    created_ms: Mapped[int] = mapped_column(Integer)

    conversation: Mapped[AdvisorConversation] = relationship(back_populates="messages")


class ImportedRun(Base):
    __tablename__ = "imported_runs"

    run_id: Mapped[str] = mapped_column(Text, primary_key=True)
    profile_id: Mapped[str | None] = mapped_column(Text)
    hero: Mapped[str] = mapped_column(Text)
    season: Mapped[int | None] = mapped_column(Integer)
    created_at: Mapped[str | None] = mapped_column(Text)
    wins: Mapped[int | None] = mapped_column(Integer)
    losses: Mapped[int | None] = mapped_column(Integer)
    victory_tier: Mapped[str | None] = mapped_column(Text)
    url: Mapped[str | None] = mapped_column(Text)

    cards: Mapped[list[ImportedRunCard]] = relationship(passive_deletes=True)
    picks: Mapped[list[ImportedPick]] = relationship(passive_deletes=True)
    combats: Mapped[list[ImportedCombat]] = relationship(passive_deletes=True)


def _run_id() -> Mapped[str]:
    return mapped_column(Text, ForeignKey("imported_runs.run_id", onupdate="CASCADE", ondelete="CASCADE"))


class ImportedRunCard(Base):
    __tablename__ = "imported_run_cards"

    rowid: Mapped[int] = mapped_column(Integer, primary_key=True)
    run_id: Mapped[str] = _run_id()
    card_id: Mapped[str] = mapped_column(Text)
    kind: Mapped[str] = mapped_column(Text)
    tier: Mapped[str | None] = mapped_column(Text)
    slot: Mapped[int | None] = mapped_column(Integer)
    size: Mapped[int | None] = mapped_column(Integer)
    enchantment: Mapped[str | None] = mapped_column(Text)


class ImportedPick(Base):
    __tablename__ = "imported_picks"

    rowid: Mapped[int] = mapped_column(Integer, primary_key=True)
    run_id: Mapped[str] = _run_id()
    day: Mapped[int | None] = mapped_column(Integer)
    hour: Mapped[int | None] = mapped_column(Integer)
    card_id: Mapped[str] = mapped_column(Text)
    tier: Mapped[str | None] = mapped_column(Text)
    # Stored as INTEGER 0 or 1.
    taken: Mapped[bool] = mapped_column(Boolean)


class ImportedCombat(Base):
    __tablename__ = "imported_combats"

    rowid: Mapped[int] = mapped_column(Integer, primary_key=True)
    run_id: Mapped[str] = _run_id()
    day: Mapped[int | None] = mapped_column(Integer)
    hour: Mapped[int | None] = mapped_column(Integer)
    outcome: Mapped[str | None] = mapped_column(Text)


class _PairStats:
    """A statistic for a pair of cards (a, b), per hero and season."""

    hero: Mapped[str] = mapped_column(Text, primary_key=True)
    season: Mapped[int] = mapped_column(Integer, primary_key=True)
    a: Mapped[str] = mapped_column(Text, primary_key=True)
    b: Mapped[str] = mapped_column(Text, primary_key=True)
    n: Mapped[int] = mapped_column(Integer)
    lift: Mapped[float | None] = mapped_column()
    win_gain: Mapped[float | None] = mapped_column()


class ImportedPairStats(_PairStats, Base):
    __tablename__ = "imported_pair_stats"


class ImportedPickStats(_PairStats, Base):
    __tablename__ = "imported_pick_stats"


class ImportedAdjacencyStats(_PairStats, Base):
    __tablename__ = "imported_adjacency_stats"


__all__ = [
    "AdvisorConversation",
    "AdvisorMessage",
    "Base",
    "ImportedAdjacencyStats",
    "ImportedCombat",
    "ImportedPairStats",
    "ImportedPick",
    "ImportedPickStats",
    "ImportedRun",
    "ImportedRunCard",
]
