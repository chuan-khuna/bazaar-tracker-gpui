"""Run one agent in the terminal, without the server or any history (copied from tau-app).

just advisor run -m "Which of these should I take?" --situation situation.json
just advisor run -m "hi" --fake        # no model: checks the plumbing, needs no key
just advisor run -m "Which?" --fake --situation situation.json   # the fake calls the tools on the Offer
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import TextIO

from tau_agent import (
    AssistantMessage,
    MessageEndEvent,
    MessageUpdateEvent,
    ToolExecutionEndEvent,
    ToolExecutionStartEvent,
)
from tau_agent.provider import ModelProvider
from tau_ai import TextDeltaEvent

import config
from agents import AGENTS, build_harness
from agents.advisor.agent import SOURCES_KEY
from agents.advisor.situation import SituationError, parse_situation
from agents.advisor.sources import Sources
from agents.core.fake import advisor_turn
from agents.core.provider import provider_from
from agents.core.registry import SITUATION_KEY, AgentSpec, MemoryState


async def run(
    spec: AgentSpec,
    message: str,
    *,
    provider: ModelProvider,
    model: str,
    situation: object | None = None,
    sources: Sources | None = None,
    out: TextIO | None = None,
) -> None:
    """Run ``spec`` on ``message`` and print the reply, tool calls and errors to ``out`` (stdout).
    ``sources``: where the tools read (default: the config's history.db and the game's catalog)."""
    out = sys.stdout if out is None else out
    state = MemoryState()
    if situation is not None:
        state.put(SITUATION_KEY, situation)
    if sources is not None:
        state.put(SOURCES_KEY, sources)
    harness = build_harness(spec, provider=provider, model=model, history=(), session_id="cli", state=state)
    async for event in harness.prompt(message):
        if isinstance(event, MessageUpdateEvent) and isinstance(event.assistant_message_event, TextDeltaEvent):
            print(event.assistant_message_event.delta, end="", flush=True, file=out)
        elif isinstance(event, ToolExecutionStartEvent):
            print(f"\n[tool] {event.tool_name}({event.args})", flush=True, file=out)
        elif isinstance(event, ToolExecutionEndEvent) and event.is_error:
            print(f"[tool error] {event.result.text}", flush=True, file=out)
        elif isinstance(event, MessageEndEvent) and isinstance(event.message, AssistantMessage):
            if event.message.error_message:
                print(f"\n[error] {event.message.error_message}", file=out)
    print(file=out)


def read_situation(path: Path) -> object:
    """The situation JSON in ``path``, checked against the schema."""
    try:
        situation = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise SystemExit(f"Can't read the situation file {path}: {error.strerror or error}") from None
    except json.JSONDecodeError as error:
        raise SystemExit(f"The situation file {path} is not JSON: {error}") from None
    try:
        parse_situation(situation)
    except SituationError as error:
        raise SystemExit(f"{path}: {error}") from None
    return situation


def main(argv: Sequence[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("agent", nargs="?", default="advisor", choices=sorted(AGENTS))
    parser.add_argument("-m", "--message", required=True)
    parser.add_argument("--situation", type=Path, help="a JSON file with the situation, as the app sends it")
    parser.add_argument("--model", help="the model id (default: ADVISOR_MODEL)")
    parser.add_argument("--fake", action="store_true", help="answer with set text instead of calling a model")
    args = parser.parse_args(argv)

    settings = config.load()
    for note in settings.notes:
        print(f"warning: {note}", file=sys.stderr)
    situation = read_situation(args.situation) if args.situation else None
    if args.fake:
        provider: ModelProvider = advisor_turn(args.message, situation)
    else:
        try:
            provider = provider_from(settings)
        except RuntimeError as error:
            raise SystemExit(str(error)) from None
    asyncio.run(
        run(
            AGENTS[args.agent],
            args.message,
            provider=provider,
            model=args.model or settings.model,
            situation=situation,
            sources=Sources.from_config(settings),
        )
    )


if __name__ == "__main__":
    main()
