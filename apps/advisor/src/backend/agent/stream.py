"""tau_agent events -> the events the app gets over SSE (copied from tau-app).

- message_start / message_end: a whole message (user, assistant or tool result).
- delta: streamed text, thinking or tool-call arguments of the current assistant message.
- step: an assistant message's step stats, right after its message_end (sent by the runner).
- stats, error, done: sent by the runner around these.

The payloads are documented in docs/documents/advisor.md ("The API").
"""

from __future__ import annotations

from typing import Any

from tau_agent import AgentEvent, MessageEndEvent, MessageStartEvent, MessageUpdateEvent
from tau_ai import TextDeltaEvent, ThinkingDeltaEvent, ToolCallDeltaEvent

from backend.db.store import dump_message

type StreamEvent = dict[str, Any]

_DELTA_KINDS = {
    TextDeltaEvent: "text",
    ThinkingDeltaEvent: "thinking",
    ToolCallDeltaEvent: "toolcall",
}


def to_stream_event(event: AgentEvent) -> StreamEvent | None:
    """The app's event for ``event``, or None for events the app doesn't need."""
    if isinstance(event, MessageStartEvent):
        return {"type": "message_start", "message": dump_message(event.message)}
    if isinstance(event, MessageEndEvent):
        return {"type": "message_end", "message": dump_message(event.message)}
    if isinstance(event, MessageUpdateEvent):
        inner = event.assistant_message_event
        kind = _DELTA_KINDS.get(type(inner))
        if kind:
            return {"type": "delta", "kind": kind, "index": inner.content_index, "delta": inner.delta}
    return None
