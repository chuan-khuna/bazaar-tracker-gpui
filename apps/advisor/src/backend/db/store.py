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
from sqlalchemy import Engine, delete, func, select, text
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
    # The reply's total, on its last assistant message only.
    stats: dict[str, Any] | None
    created_ms: int
    # This message's step (one model call), on every assistant message saved since step stats began.
    step: dict[str, Any] | None = None
    # The situation the app sent with this question, on a user message sent with one.
    situation: dict[str, Any] | None = None

    def to_json(self) -> dict[str, Any]:
        return {
            "seq": self.seq,
            "message": dump_message(self.message),
            "stats": self.stats,
            "step": self.step,
            "situation": self.situation,
            "createdMs": self.created_ms,
        }

    @classmethod
    def from_row(cls, row: AdvisorMessage) -> StoredMessage:
        """The row's message, and its ``stats`` column split into the reply total, the step and the
        situation."""
        total, step = split_stats(row.stats)
        situation = row.stats.get(SITUATION) if isinstance(row.stats, dict) else None
        return cls(
            row.seq,
            _MESSAGE.validate_python(row.message),  # type: ignore[arg-type]
            total,
            row.created_ms,
            step,
            situation if isinstance(situation, dict) else None,
        )


# The key a question's situation is saved under in advisor_messages.stats (it has no column).
SITUATION = "situation"


def split_stats(column: object) -> tuple[dict[str, Any] | None, dict[str, Any] | None]:
    """advisor_messages.stats as (the reply total, the step). The column holds the step under
    ``step`` and, on a reply's last message, the total's fields beside it; a row saved before step
    stats holds the total alone. A question's ``situation`` there is neither."""
    if not isinstance(column, dict):
        return None, None
    step = column.get("step")
    total = {key: value for key, value in column.items() if key not in ("step", SITUATION)}
    return total or None, step if isinstance(step, dict) else None


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
            return [StoredMessage.from_row(row) for row in s.scalars(query)]

    def append(
        self,
        conversation_id: str,
        message: AgentMessage,
        step: dict[str, Any] | None = None,
        situation: object | None = None,
    ) -> int:
        """Stores ``message`` after the Conversation's others, with its ``step`` stats when it has
        them (an assistant message) or the ``situation`` sent with it (a question), and returns its
        seq. Both go in the ``stats`` JSON column."""
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
                    stats=_stats_column(step, situation),
                    created_ms=now_ms(),
                )
            )
        return seq

    def set_stats(self, conversation_id: str, seq: int, stats: dict[str, Any]) -> None:
        """Saves the reply total ``stats`` on the message at ``seq``, beside its step stats."""
        with self._session.begin() as s:
            row = s.get(AdvisorMessage, (conversation_id, seq))
            if row is None:
                return
            _, step = split_stats(row.stats)
            row.stats = {**stats, "step": step} if step is not None else dict(stats)


def _stats_column(step: dict[str, Any] | None, situation: object | None) -> dict[str, Any] | None:
    column: dict[str, Any] = {}
    if step is not None:
        column["step"] = step
    if situation is not None:
        column[SITUATION] = situation
    return column or None


def sendable_messages(stored: Sequence[StoredMessage]) -> list[tuple[AgentMessage, dict[str, Any] | None]]:
    """The messages to send the model, each with the situation its question was sent with (None on
    the rest): all of them except failed replies, which carry no content."""
    return [
        (s.message, s.situation)
        for s in stored
        if not (getattr(s.message, "role", None) == "assistant" and s.message.stop_reason == "error")
    ]
