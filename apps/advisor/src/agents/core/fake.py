"""A provider that answers with set replies and calls no model: for tests and `advisor --fake`.

A reply is text, or a tool call (``tool_reply``). With an Offer in the situation, ``--fake`` plays
the Advisor's usual turn: rate_options on the Offer, suggest with the Offer in its order,
follow_ups, then text. The tools really run, so this checks them end to end without a key."""

from __future__ import annotations

from typing import Any

from tau_agent.messages import AssistantMessage, ToolCall
from tau_agent.provider_events import AssistantDoneEvent, AssistantStartEvent, TextDeltaEvent, ToolCallEndEvent
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


def tool_reply(name: str, arguments: dict[str, Any], call_id: str | None = None, model: str = "fake") -> list[object]:
    """The events of one assistant message that calls the tool ``name`` with ``arguments``."""
    call = ToolCall(id=call_id or f"call-{name}", name=name, arguments=arguments)
    return [
        AssistantStartEvent(partial=AssistantMessage(model=model)),
        ToolCallEndEvent(content_index=0, tool_call=call, partial=AssistantMessage(content=[call])),
        AssistantDoneEvent(
            reason="toolUse", message=AssistantMessage(content=[call], model=model, stop_reason="toolUse")
        ),
    ]


def advisor_turn(message: str, situation: object | None) -> FakeProvider:
    """``--fake``: with an Offer in ``situation``, rate_options (shop_plan for an Offer of encounters
    only), suggest (the Offer in order) and follow_ups before the text; otherwise the text alone."""
    text = f"(fake provider, no model called) You asked: {message}"
    offer = situation.get("offer") if isinstance(situation, dict) else None
    cards = [card for card in offer or () if isinstance(card, dict) and isinstance(card.get("name"), str)]
    names = [card["name"] for card in cards]
    if not names:
        return fake_provider(text)
    ranked = [{"card": name, "reason": "fake provider: in the Offer's order"} for name in names]
    encounters_only = all(card.get("kind") == "encounter_choice" for card in cards)
    return FakeProvider(
        [
            tool_reply("shop_plan" if encounters_only else "rate_options", {}),
            tool_reply("suggest", {"title": "Fake ranking", "ranked": ranked}),
            tool_reply("follow_ups", {"questions": ["Where should I put it?", "Which skill fits this Board?"]}),
            text_reply(text),
        ]
    )
