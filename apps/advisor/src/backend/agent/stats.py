"""Stats for one reply and for each of its steps: tokens, cost and time.

A step is one model call, which ends in one assistant message. Its stats are the message's usage,
the model that answered, and the time from the call's start (the end of the message before it: the
question or the last tool result) to the message's end. The reply's stats add up its steps' tokens,
cost and tool calls; its time runs from the question to the end of the last step, tool calls included.

Copied from tau-app without its OpenRouter price list: the cost is what the provider reports
(OpenRouter reports it in the stream; a local model reports none, so 0).
"""

from __future__ import annotations

import time
from collections.abc import Callable, Iterable
from typing import Any

from tau_agent import AssistantMessage
from tau_agent.messages import Usage

# A model name tau_agent uses when the provider gave none.
_UNKNOWN_MODEL = "unknown"


class ReplyStats:
    def __init__(self, model: str, clock: Callable[[], float] = time.monotonic) -> None:
        self._model = model
        self._clock = clock
        self._started = clock()
        self._step_started = self._started
        self._messages: list[AssistantMessage] = []

    def step_starts(self) -> None:
        """A message before the next model call ended: the next step's time counts from now."""
        self._step_started = self._clock()

    def add(self, message: AssistantMessage) -> dict[str, Any]:
        """Counts ``message`` in the reply, and returns its step's stats (``step`` in advisor_messages.stats
        and in the `step` event)."""
        now = self._clock()
        elapsed = now - self._step_started
        self._step_started = now
        self._messages.append(message)
        model = message.response_model or message.model
        return _stats(
            model if model and model != _UNKNOWN_MODEL else self._model,
            elapsed,
            [message.usage],
            len(message.tool_calls),
        )

    def finish(self) -> dict[str, Any]:
        """The reply's stats as JSON, as stored in advisor_messages.stats and sent in the `stats` event."""
        return _stats(
            self._model,
            self._clock() - self._started,
            [m.usage for m in self._messages],
            sum(len(m.tool_calls) for m in self._messages),
        )


def _stats(model: str, seconds: float, usages: Iterable[Usage], tool_calls: int) -> dict[str, Any]:
    usages = list(usages)
    return {
        "model": model,
        "elapsedMs": round(seconds * 1000),
        "tokens": {
            "input": sum(u.input for u in usages),
            "output": sum(u.output for u in usages),
            "cacheRead": sum(u.cache_read for u in usages),
            "cacheWrite": sum(u.cache_write for u in usages),
            "total": sum(u.total_tokens for u in usages),
        },
        "costUsd": sum(u.cost.total for u in usages),
        "pricingSource": "provider",
        "toolCalls": tool_calls,
    }
