"""A Conversation's messages: the stored history, and sending a message (its reply streamed as SSE)."""

from __future__ import annotations

import json
from collections.abc import AsyncIterator
from contextlib import aclosing
from typing import Any

from fastapi import APIRouter, HTTPException
from fastapi.responses import StreamingResponse

from backend.agent.stream import StreamEvent
from backend.deps import ConversationDep, RunsDep, StoreDep
from backend.models import checked_model
from backend.schemas import SendMessage, checked_situation, question

router = APIRouter(prefix="/conversations/{conversation_id}", tags=["messages"])


@router.get("/messages")
def get_messages(conversation: ConversationDep, store: StoreDep, runs: RunsDep) -> dict[str, Any]:
    return {
        "running": runs.is_running(conversation.id),
        "messages": [m.to_json() for m in store.messages(conversation.id)],
    }


def sse_frame(event: StreamEvent) -> str:
    """One SSE event: its ``type`` as the event name, the whole event as one line of JSON data."""
    return f"event: {event['type']}\ndata: {json.dumps(event, ensure_ascii=False)}\n\n"


@router.post("/messages")
async def send_message(conversation: ConversationDep, body: SendMessage, runs: RunsDep) -> StreamingResponse:
    """Runs the Advisor on the message; the response streams its events (backend/agent/stream.py).
    Closing the response cancels the reply. ``model``, when given, answers this message instead of
    the config's ADVISOR_MODEL (same provider and key)."""
    text = question(body.text)
    checked_situation(body.situation)
    model = checked_model(body.model)
    if runs.is_running(conversation.id):
        raise HTTPException(409, "A reply is still running.")

    async def sse() -> AsyncIterator[str]:
        # aclosing: a closed response closes the reply at once, which cancels it.
        async with aclosing(runs.run_message(conversation, text, body.situation, model)) as events:
            async for event in events:
                yield sse_frame(event)

    return StreamingResponse(
        sse(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )
