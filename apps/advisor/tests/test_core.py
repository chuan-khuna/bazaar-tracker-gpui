"""tau-app's core as copied: define_tool's argument checks, ends_reply, ask_user, and build_harness."""

from __future__ import annotations

import asyncio
from typing import Any

import pytest
from tau_agent import ToolResultMessage

from agents.common.tools import ask_user
from agents.core.fake import fake_provider
from agents.core.registry import MAX_TURNS, AgentSpec, MemoryState, RunSetup, build_harness
from agents.core.tooling import define_tool, ends_reply


async def _echo(args: dict[str, Any]) -> dict[str, Any]:
    return args


ECHO = define_tool(
    name="echo",
    description="Echo the arguments.",
    properties={
        "text": {"type": "string"},
        "count": {"type": "integer"},
        "mode": {"type": "string", "enum": ["a", "b"]},
    },
    required=("text",),
    run=_echo,
)


def execute(tool: Any, arguments: dict[str, Any]) -> Any:
    return asyncio.run(tool.execute("call-1", arguments))


def test_define_tool_passes_checked_arguments_and_returns_json() -> None:
    result = execute(ECHO, {"text": "hi", "count": 2, "mode": None})
    assert result.details == {"text": "hi", "count": 2}
    assert result.content[0].text == '{"text": "hi", "count": 2}'
    assert ECHO.parameters["required"] == ["text"]
    assert ECHO.parameters["additionalProperties"] is False


@pytest.mark.parametrize(
    ("arguments", "message"),
    [
        ({}, "Missing required argument"),
        ({"text": "hi", "other": 1}, "Unknown argument"),
        ({"text": 3}, "must be a string"),
        ({"text": "hi", "count": True}, "must be a integer"),
        ({"text": "hi", "mode": "c"}, "must be one of: a, b"),
    ],
)
def test_define_tool_rejects_bad_arguments(arguments: dict[str, Any], message: str) -> None:
    with pytest.raises(ValueError, match=message):
        execute(ECHO, arguments)


def _result(name: str, is_error: bool = False) -> ToolResultMessage:
    return ToolResultMessage(tool_call_id="call-1", tool_name=name, is_error=is_error)


def test_only_a_successful_terminating_tool_ends_the_reply() -> None:
    assert ends_reply([_result("echo"), _result("ask_user")])
    assert not ends_reply([_result("ask_user", is_error=True)])
    assert not ends_reply([_result("echo")])


def test_ask_user_waits_for_the_user() -> None:
    options = [{"id": "a", "label": "Take it"}, {"id": "b", "label": "Skip"}]
    result = execute(ask_user, {"question": "Take the Cutlass?", "options": options})
    assert result.details == {"status": "waiting_for_user"}
    assert result.terminate is True


@pytest.mark.parametrize(
    ("arguments", "message"),
    [
        ({"question": " ", "options": [{"id": "a", "label": "A"}, {"id": "b", "label": "B"}]}, "must not be empty"),
        ({"question": "Q?", "options": [{"id": "a", "label": "A"}]}, "2-6 choices"),
        ({"question": "Q?", "options": [{"id": "a", "label": ""}, {"id": "b", "label": "B"}]}, "non-empty"),
    ],
)
def test_ask_user_rejects_bad_questions(arguments: dict[str, Any], message: str) -> None:
    with pytest.raises(ValueError, match=message):
        execute(ask_user, arguments)


def test_build_harness_uses_prepare_with_the_run_state() -> None:
    seen: list[object] = []

    def prepare(context: Any) -> RunSetup:
        seen.append((context.session_id, context.state.get("situation")))
        return RunSetup(f"day {context.state.get('situation')['day']}", (ECHO,))

    spec = AgentSpec(id="t", name="T", description="", examples=(), system="unused", tools=(), prepare=prepare)
    state = MemoryState()
    state.put("situation", {"day": 4})
    provider = fake_provider("ok")

    harness = build_harness(spec, provider=provider, model="m", history=(), session_id="s1", state=state)
    asyncio.run(_drain(harness.prompt("hi")))

    assert seen == [("s1", {"day": 4})]
    model, system, _messages, tools = provider.calls[0]
    assert (model, system, [tool.name for tool in tools]) == ("m", "day 4", ["echo"])
    assert harness.config.max_turns == MAX_TURNS


def test_a_spec_without_prepare_uses_its_prompt_and_tools() -> None:
    spec = AgentSpec(id="t", name="T", description="d", examples=("e",), system="sys", tools=(ECHO,))
    provider = fake_provider("ok")
    harness = build_harness(spec, provider=provider, model="m", history=(), session_id="s", state=MemoryState())
    asyncio.run(_drain(harness.prompt("hi")))
    assert provider.calls[0][1] == "sys"
    assert spec.info() == {"id": "t", "name": "T", "description": "d", "examples": ["e"]}


async def _drain(events: Any) -> None:
    async for _ in events:
        pass
