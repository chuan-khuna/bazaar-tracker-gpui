"""Conversations and their messages in history.db (copied from tau-app's db/history.py, on the
advisor_* tables).

``Store`` hands the routers and the runner frozen dataclasses (``Conversation``,
``StoredMessage``) rather than ORM rows, so nothing outside db/ holds a session. Every method runs
in its own short transaction, so the tracker's writes are never held up for long.

A Conversation belongs to a Run (``run_tracker_runs``, the tracker's): the schema's cascades move
it when the Run is rebound and delete it with the Run. Times are Unix ms, as in the tracker's tables.
"""

from __future__ import annotations

import time
import uuid
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

from pydantic import TypeAdapter
from sqlalchemy import Engine, delete, func, select, text, update
from tau_agent import AgentMessage

from backend.db.connection import sessions
from backend.db.models import AdvisorConversation, AdvisorMessage

# The longest title: the first words of the first question, cut on a word.
TITLE_MAX_LENGTH = 48

_MESSAGE = TypeAdapter(AgentMessage)


@dataclass(frozen=True, slots=True)
class Conversation:
    id: str
    run_id: str
    title: str
    day: int | None
    hour: int | None
    created_ms: int

    def to_json(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "runId": self.run_id,
            "title": self.title,
            "day": self.day,
            "hour": self.hour,
            "createdMs": self.created_ms,
        }

    @classmethod
    def from_row(cls, row: AdvisorConversation) -> Conversation:
        return cls(row.id, row.run_id, row.title, row.day, row.hour, row.created_ms)


@dataclass(frozen=True, slots=True)
class StoredMessage:
    seq: int
    message: AgentMessage
    stats: dict[str, Any] | None
    created_ms: int

    def to_json(self) -> dict[str, Any]:
        return {
            "seq": self.seq,
            "message": dump_message(self.message),
            "stats": self.stats,
            "createdMs": self.created_ms,
        }


def dump_message(message: AgentMessage) -> dict[str, Any]:
    """The message's tau_agent JSON wire form (camelCase), as stored and as streamed."""
    return _MESSAGE.dump_python(message, mode="json", by_alias=True, exclude_none=True)


def now_ms() -> int:
    return int(time.time() * 1000)


def title_from(question: str) -> str:
    """The question's first words on one line, cut on a word to ``TITLE_MAX_LENGTH`` with an ellipsis."""
    one_line = " ".join(question.split())
    if len(one_line) <= TITLE_MAX_LENGTH:
        return one_line
    cut = one_line[: TITLE_MAX_LENGTH - 1]
    if " " in cut:
        cut = cut.rsplit(" ", 1)[0]
    return cut.rstrip(" ,.;:?!") + "…"


class Store:
    """Each Run's Conversations and each Conversation's messages."""

    def __init__(self, engine: Engine) -> None:
        self._session = sessions(engine)

    # Runs (the tracker's; read only)

    def run_exists(self, run_id: str) -> bool:
        with self._session() as s:
            return s.execute(text("SELECT 1 FROM run_tracker_runs WHERE id = :id"), {"id": run_id}).first() is not None

    # Conversations

    def conversations(self, run_id: str) -> list[Conversation]:
        """The Run's Conversations, newest first."""
        query = (
            select(AdvisorConversation)
            .where(AdvisorConversation.run_id == run_id)
            .order_by(AdvisorConversation.created_ms.desc(), AdvisorConversation.id.desc())
        )
        with self._session() as s:
            return [Conversation.from_row(row) for row in s.scalars(query)]

    def get(self, conversation_id: str) -> Conversation | None:
        with self._session() as s:
            row = s.get(AdvisorConversation, conversation_id)
            return Conversation.from_row(row) if row else None

    def create(self, run_id: str, first_question: str, day: int | None, hour: int | None) -> Conversation:
        """A new Conversation of ``run_id``, titled by its first question, at the situation's Day and Hour."""
        row = AdvisorConversation(
            id=str(uuid.uuid4()),
            run_id=run_id,
            title=title_from(first_question),
            day=day,
            hour=hour,
            created_ms=now_ms(),
        )
        with self._session.begin() as s:
            s.add(row)
        return Conversation.from_row(row)

    def delete(self, conversation_id: str) -> None:
        """Deletes the Conversation; its messages go by the schema's cascade."""
        with self._session.begin() as s:
            s.execute(delete(AdvisorConversation).where(AdvisorConversation.id == conversation_id))

    # Messages

    def messages(self, conversation_id: str) -> list[StoredMessage]:
        query = (
            select(AdvisorMessage).where(AdvisorMessage.conversation_id == conversation_id).order_by(AdvisorMessage.seq)
        )
        with self._session() as s:
            return [
                StoredMessage(row.seq, _MESSAGE.validate_python(row.message), row.stats, row.created_ms)  # type: ignore[arg-type]
                for row in s.scalars(query)
            ]

    def append(self, conversation_id: str, message: AgentMessage) -> int:
        """Stores ``message`` after the Conversation's others and returns its seq."""
        with self._session.begin() as s:
            last = s.scalar(
                select(func.max(AdvisorMessage.seq)).where(AdvisorMessage.conversation_id == conversation_id)
            )
            seq = 0 if last is None else last + 1
            s.add(
                AdvisorMessage(
                    conversation_id=conversation_id,
                    seq=seq,
                    role=message.role,
                    message=dump_message(message),
                    created_ms=now_ms(),
                )
            )
        return seq

    def set_stats(self, conversation_id: str, seq: int, stats: dict[str, Any]) -> None:
        with self._session.begin() as s:
            s.execute(
                update(AdvisorMessage)
                .where(AdvisorMessage.conversation_id == conversation_id, AdvisorMessage.seq == seq)
                .values(stats=stats)
            )


def model_history(stored: Sequence[StoredMessage]) -> list[AgentMessage]:
    """The messages to send the model: all of them except failed replies, which carry no content."""
    return [
        s.message
        for s in stored
        if not (getattr(s.message, "role", None) == "assistant" and s.message.stop_reason == "error")
    ]
