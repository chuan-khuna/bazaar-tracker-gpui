"""Stats for one reply: tokens, cost and time over its assistant messages (one per model turn).

Copied from tau-app without its OpenRouter price list: the cost is what the provider reports
(OpenRouter reports it in the stream; a local model reports none, so 0).
"""

from __future__ import annotations

import time
from typing import Any

from tau_agent import AssistantMessage


class ReplyStats:
    def __init__(self, model: str) -> None:
        self._model = model
        self._started = time.monotonic()
        self._messages: list[AssistantMessage] = []

    def add(self, message: AssistantMessage) -> None:
        self._messages.append(message)

    def finish(self) -> dict[str, Any]:
        """The stats as JSON, as stored in advisor_messages.stats and sent in the `stats` event."""
        usages = [m.usage for m in self._messages]
        return {
            "model": self._model,
            "elapsedMs": round((time.monotonic() - self._started) * 1000),
            "tokens": {
                "input": sum(u.input for u in usages),
                "output": sum(u.output for u in usages),
                "cacheRead": sum(u.cache_read for u in usages),
                "cacheWrite": sum(u.cache_write for u in usages),
                "total": sum(u.total_tokens for u in usages),
            },
            "costUsd": sum(u.cost.total for u in usages),
            "pricingSource": "provider",
            "toolCalls": sum(len(m.tool_calls) for m in self._messages),
        }
