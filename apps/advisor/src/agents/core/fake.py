"""A provider that answers with set text and calls no model: for tests and `advisor --fake`."""

from __future__ import annotations

from tau_agent.messages import AssistantMessage
from tau_agent.provider_events import AssistantDoneEvent, AssistantStartEvent, TextDeltaEvent
from tau_ai import FakeProvider


def text_reply(text: str, model: str = "fake") -> list[object]:
    """The events of one assistant message that says ``text``."""
    return [
        AssistantStartEvent(partial=AssistantMessage(model=model)),
        TextDeltaEvent(content_index=0, delta=text, partial=AssistantMessage(content=text)),
        AssistantDoneEvent(reason="stop", message=AssistantMessage(content=text, model=model, stop_reason="stop")),
    ]


def fake_provider(*replies: str) -> FakeProvider:
    """A provider that answers each model call with the next of ``replies``."""
    return FakeProvider([text_reply(text) for text in replies])
