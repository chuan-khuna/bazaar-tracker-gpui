"""Request bodies, and checking the situation they carry."""

from __future__ import annotations

from typing import Any

from fastapi import HTTPException
from pydantic import BaseModel

from agents.advisor.situation import Situation, SituationError, parse_situation


class StartConversation(BaseModel):
    # The first question: the Conversation's title is its first words.
    text: str
    # The situation now: the Conversation keeps its Day and Hour.
    situation: dict[str, Any] | None = None


class SendMessage(BaseModel):
    text: str
    # The situation the app sends with every message; it goes into the system prompt, never stored.
    situation: dict[str, Any] | None = None
    # The model to answer this message with (checked by ``backend.models.checked_model``); None: the
    # config's ADVISOR_MODEL. Any, so a wrong type is a 400 like a malformed id.
    model: Any = None


def question(text: str) -> str:
    """``text`` stripped; 400 when empty."""
    text = text.strip()
    if not text:
        raise HTTPException(400, "Message is empty.")
    return text


def checked_situation(situation: dict[str, Any] | None) -> Situation | None:
    """The situation checked against the schema; 400 naming each bad field."""
    if situation is None:
        return None
    try:
        return parse_situation(situation)
    except SituationError as error:
        raise HTTPException(400, str(error)) from None
