"""Running one user message through the Advisor, streamed to the app (copied from tau-app).

``Runs.run_message`` loads the Conversation's stored messages, has the agent rewrite them for the
model given the situation now (``AgentSpec.model_history``), builds a tau_agent harness with the
situation in its state, prompts it with the question beside its situation (``question_message``)
and yields stream events (``stream.py``), each assistant message's step stats after its
message_end, then the reply's stats. The question is stored as typed, with its situation beside it.

Every message is saved as it ends, through a harness subscriber, so the history survives a closed
stream. Closing the stream cancels the run: the assistant message still streaming is saved as far as
it got (its text and thinking, ``stopReason: "aborted"``), and a tool call left without a result
gets "Tool call interrupted by user". Changed from tau-app: the situation and the tools' ``Sources``
go into the run's state; the provider is made per message (``ProviderFor``), so the fake provider can
read the message and the situation; a stopped reply keeps its partial message.
"""

from __future__ import annotations

import json
import logging
from collections.abc import AsyncIterator, Callable, Sequence

from tau_agent import (
    AgentEvent,
    AgentHarness,
    AgentMessage,
    AssistantMessage,
    MessageEndEvent,
    MessageStartEvent,
    MessageUpdateEvent,
    TextContent,
    ThinkingContent,
    ToolResultMessage,
    TurnEndEvent,
    UserMessage,
    message_text,
)
from tau_agent.provider import ModelProvider

from agents import AGENTS, build_harness
from agents.advisor.agent import SOURCES_KEY
from agents.advisor.sources import Sources
from agents.core.registry import SITUATION_KEY, MemoryState
from agents.core.tooling import ends_reply
from backend.agent.stats import ReplyStats
from backend.agent.stream import StreamEvent, to_stream_event
from backend.db.store import Conversation, Store, sendable_messages

log = logging.getLogger(__name__)

# The provider for one message: given the question and the situation (the fake provider reads both).
type ProviderFor = Callable[[str, object | None], ModelProvider]

AGENT_ID = "advisor"


class Runs:
    """The replies in progress, at most one per Conversation."""

    def __init__(self, store: Store, sources: Sources, provider_for: ProviderFor, model: str) -> None:
        self._store = store
        self._sources = sources
        self._provider_for = provider_for
        self._model = model
        self._running: dict[str, AgentHarness] = {}

    def is_running(self, conversation_id: str) -> bool:
        return conversation_id in self._running

    def cancel(self, conversation_id: str) -> bool:
        harness = self._running.get(conversation_id)
        if harness:
            harness.cancel()
        return harness is not None

    async def run_message(
        self, conversation: Conversation, text: str, situation: object | None = None, model: str | None = None
    ) -> AsyncIterator[StreamEvent]:
        """The reply's events, answered by ``model`` (None: the default). A Conversation already
        replying gets an ``error`` and ``done``."""
        model = model or self._model
        if conversation.id in self._running:
            yield {"type": "error", "error": "A reply is still running."}
            yield {"type": "done"}
            return
        spec = AGENTS[AGENT_ID]
        history = spec.model_history(sendable_messages(self._store.messages(conversation.id)), situation)
        state = MemoryState()
        if situation is not None:
            state.put(SITUATION_KEY, situation)
        state.put(SOURCES_KEY, self._sources)
        harness = build_harness(
            spec,
            provider=self._provider_for(text, situation),
            model=model,
            history=history,
            session_id=conversation.id,
            state=state,
        )
        log.info(
            "[%s] reply: model=%s history=%d message(s) tools=[%s]",
            conversation.id,
            model,
            len(history),
            ", ".join(t.name for t in harness.config.tools),
        )
        stats = ReplyStats(model)
        last_assistant_seq: int | None = None
        # The assistant message streaming now, until its message_end.
        partial: AssistantMessage | None = None
        # The `step` event of the assistant message that just ended, sent after its message_end.
        step_event: StreamEvent | None = None

        def save(event: AgentEvent) -> None:
            nonlocal last_assistant_seq, partial, step_event
            if isinstance(event, MessageUpdateEvent) and isinstance(event.message, AssistantMessage):
                partial = event.message
            elif isinstance(event, MessageEndEvent):
                if isinstance(event.message, AssistantMessage):
                    step = stats.add(event.message)
                    seq = self._store.append(conversation.id, event.message, step)
                    partial = None
                    last_assistant_seq = seq
                    step_event = {"type": "step", "seq": seq, "step": step}
                else:
                    # The question is saved as typed, keeping the situation sent with it: a reloaded
                    # Conversation shows both, and the next question's history marks it.
                    if isinstance(event.message, UserMessage):
                        self._store.append(conversation.id, typed, situation=situation)
                    else:
                        self._store.append(conversation.id, event.message)
                    # The question or a tool result: the next model call starts now.
                    stats.step_starts()

        # The question as the player typed it: what is saved and streamed to the app. The model gets
        # it with the situation above it.
        typed = UserMessage(content=text)
        unsubscribe = harness.subscribe(save)
        self._running[conversation.id] = harness
        events = harness.prompt_message(spec.question_message(text, situation))
        # Whether an assistant message of this reply has said something: follow_ups then ends it.
        answered = False
        try:
            async for event in events:
                if isinstance(event, (MessageStartEvent, MessageEndEvent)) and isinstance(event.message, UserMessage):
                    event = event.model_copy(update={"message": typed})
                out = to_stream_event(event)
                if out:
                    yield out
                if step_event is not None:
                    yield step_event
                    step_event = None
                # A terminating tool (ask_user), or follow_ups once the reply has text, ends the
                # reply: leaving the loop closes the harness before its next model call. Every
                # message so far is already saved.
                if isinstance(event, TurnEndEvent):
                    answered = answered or bool(message_text(event.message).strip())
                    if ends_reply(event.tool_results, answered):
                        log.info("[%s] the reply is finished; ending it", conversation.id)
                        break
            if last_assistant_seq is not None:
                result = stats.finish()
                self._store.set_stats(conversation.id, last_assistant_seq, result)
                yield {"type": "stats", "seq": last_assistant_seq, "stats": result}
        except Exception as exc:  # noqa: BLE001 - report to the app, keep serving
            log.exception("Reply failed for conversation %s", conversation.id)
            yield {"type": "error", "error": str(exc) or type(exc).__name__}
        finally:
            # A closed stream lands here without the loop finishing. Cancel before closing the
            # harness, so it records interrupted tool calls (saved by ``save``), then keep the
            # message that was still streaming.
            harness.cancel()
            await events.aclose()
            unsubscribe()
            if partial is not None:
                kept = _stopped(partial)
                if kept is not None:
                    self._store.append(conversation.id, kept)
                    log.info("[%s] stopped; kept the partial reply", conversation.id)
            self._running.pop(conversation.id, None)
            log.debug("[%s] history:\n%s", conversation.id, _describe(harness.messages))
        yield {"type": "done"}


def _stopped(partial: AssistantMessage) -> AssistantMessage | None:
    """The text and thinking that arrived of a message cut short, marked aborted; None when nothing did."""
    content = [
        block
        for block in partial.content
        if (isinstance(block, TextContent) and block.text) or (isinstance(block, ThinkingContent) and block.thinking)
    ]
    if not content:
        return None
    return partial.model_copy(update={"content": content, "stop_reason": "aborted"})


# Longest text shown per message in the history log.
_PREVIEW_CHARS = 200


def _describe(messages: Sequence[AgentMessage]) -> str:
    """One line per message for the log: index, role, and a short preview of its content."""
    lines = []
    for i, message in enumerate(messages):
        if isinstance(message, AssistantMessage):
            parts = [f"stop={message.stop_reason}"]
            if message.thinking_text:
                parts.append(f"thinking={_preview(message.thinking_text)!r}")
            if message.text:
                parts.append(f"text={_preview(message.text)!r}")
            parts += [f"call {c.name}({json.dumps(c.arguments, ensure_ascii=False)})" for c in message.tool_calls]
            if message.error_message:
                parts.append(f"error={message.error_message!r}")
            detail = " ".join(parts)
        elif isinstance(message, ToolResultMessage):
            status = "error" if message.is_error else "ok"
            detail = f"{message.tool_name} {status}: {_preview(message.text)!r}"
        else:
            detail = repr(_preview(message_text(message)))
        lines.append(f"  #{i} {message.role}: {detail}")
    return "\n".join(lines)


def _preview(text: str) -> str:
    one_line = " ".join(text.split())
    return one_line if len(one_line) <= _PREVIEW_CHARS else one_line[: _PREVIEW_CHARS - 1] + "…"
