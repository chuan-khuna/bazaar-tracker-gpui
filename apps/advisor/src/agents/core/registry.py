"""What an agent is, and building a tau_agent harness that runs one (copied from tau-app).

An ``AgentSpec`` is pure data: identity, prompt and tools. ``build_harness`` adds
the runtime parts (provider, model, the conversation so far) per run.

An agent whose prompt or tools depend on the run sets ``prepare``: it gets a
``RunContext`` at the start of every run and returns the system prompt and tools
for that run. tau_agent fixes both for the whole run. The Advisor's ``prepare``
builds its tools around the situation the app sent (kept in ``state`` under
``SITUATION_KEY``).

An agent that sends more than the typed text sets ``question`` (the message the
model gets for a question and its situation) and ``history`` (the stored
Conversation rewritten for the model, given the situation now). The Advisor puts
the situation beside each question and marks what an older situation said as old.

tau-app's learning topics (``topics``, ``TOPIC_TITLE_KEY``) are left out: the
Advisor's Conversations belong to a Run instead.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from typing import Any, Protocol

from tau_agent import AgentHarness, AgentHarnessConfig, AgentMessage, AgentTool, UserMessage
from tau_agent.provider import ModelProvider

# Stops a model that keeps calling tools without ever answering.
MAX_TURNS = 12

# State key holding the situation (a JSON object) the app sent with the message.
SITUATION_KEY = "situation"


class AgentState(Protocol):
    """JSON values by key for one run (the situation, and whatever else an agent keeps)."""

    def get(self, key: str) -> Any | None: ...

    def put(self, key: str, value: Any) -> None: ...


@dataclass(slots=True)
class MemoryState:
    """An ``AgentState`` that lives only as long as the process."""

    values: dict[str, Any] = field(default_factory=dict)

    def get(self, key: str) -> Any | None:
        return self.values.get(key)

    def put(self, key: str, value: Any) -> None:
        self.values[key] = value


@dataclass(frozen=True, slots=True)
class RunContext:
    """What ``prepare`` gets for one run."""

    session_id: str
    state: AgentState


@dataclass(frozen=True, slots=True)
class RunSetup:
    """The system prompt and tools for one run."""

    system: str
    tools: tuple[AgentTool, ...]


@dataclass(frozen=True, slots=True)
class AgentSpec:
    # The key the CLI and the backend know the agent by.
    id: str
    name: str
    description: str
    # Example questions.
    examples: tuple[str, ...]
    system: str
    tools: tuple[AgentTool, ...]
    # Per-run prompt and tools; None uses system and tools as they are.
    prepare: Callable[[RunContext], RunSetup] | None = None
    # The message the model gets for a question and the situation sent with it; None: the text alone.
    question: Callable[[str, object | None], AgentMessage] | None = None
    # The stored messages, each with the situation its question was sent with, as the model gets
    # them, given the situation now; None: the messages as they are.
    history: Callable[[Sequence[tuple[AgentMessage, object | None]], object | None], list[AgentMessage]] | None = None

    def question_message(self, text: str, situation: object | None) -> AgentMessage:
        return self.question(text, situation) if self.question else UserMessage(content=text)

    def model_history(
        self, stored: Sequence[tuple[AgentMessage, object | None]], situation: object | None
    ) -> list[AgentMessage]:
        return self.history(stored, situation) if self.history else [message for message, _ in stored]

    def info(self) -> dict[str, object]:
        """The public fields."""
        return {
            "id": self.id,
            "name": self.name,
            "description": self.description,
            "examples": list(self.examples),
        }

    def setup(self, context: RunContext) -> RunSetup:
        """The system prompt and tools for a run."""
        return self.prepare(context) if self.prepare else RunSetup(self.system, self.tools)


def build_harness(
    spec: AgentSpec,
    *,
    provider: ModelProvider,
    model: str,
    history: Sequence[AgentMessage],
    session_id: str,
    state: AgentState,
) -> AgentHarness:
    """A harness for one run of ``spec``, continuing from ``history``."""
    setup = spec.setup(RunContext(session_id=session_id, state=state))
    config = AgentHarnessConfig(
        provider=provider,
        model=model,
        system=setup.system,
        tools=list(setup.tools),
        max_turns=MAX_TURNS,
        session_id=session_id,
    )
    return AgentHarness(config, messages=history)
