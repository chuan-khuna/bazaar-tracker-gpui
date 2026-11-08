"""The terminal runner with a fake provider: the reply, tool lines, and the situation in the prompt."""

from __future__ import annotations

import asyncio
import io
import json
from pathlib import Path

import pytest
from tau_agent.messages import AssistantMessage, ToolCall
from tau_agent.provider_events import AssistantDoneEvent, AssistantStartEvent, ToolCallEndEvent
from tau_ai import FakeProvider

from agents import AGENTS
from agents.core import cli
from agents.core.fake import fake_provider, text_reply


def run(provider: FakeProvider, message: str, situation: object | None = None) -> str:
    out = io.StringIO()
    asyncio.run(cli.run(AGENTS["advisor"], message, provider=provider, model="fake", situation=situation, out=out))
    return out.getvalue()


def test_prints_the_reply() -> None:
    provider = fake_provider("Take the Cutlass.")
    assert run(provider, "hi") == "Take the Cutlass.\n"
    _model, system, messages, tools = provider.calls[0]
    assert "The app sent no situation" in system
    assert messages[0].role == "user"
    assert [tool.name for tool in tools] == ["ask_user"]


def test_puts_the_situation_in_the_system_prompt() -> None:
    provider = fake_provider("ok")
    run(provider, "Which?", situation={"hero": "Vanessa", "day": 5, "offer": [{"name": "Cutlass"}]})
    system = provider.calls[0][1]
    assert "## Situation" in system
    assert '"hero": "Vanessa"' in system
    assert '"name": "Cutlass"' in system


def test_prints_tool_calls_and_tool_errors() -> None:
    bad = ToolCall(id="call-1", name="ask_user", arguments={"question": "Which?", "options": []})
    tool_turn = [
        AssistantStartEvent(partial=AssistantMessage(model="fake")),
        ToolCallEndEvent(content_index=0, tool_call=bad, partial=AssistantMessage(content=[bad])),
        AssistantDoneEvent(
            reason="toolUse", message=AssistantMessage(content=[bad], model="fake", stop_reason="toolUse")
        ),
    ]
    provider = FakeProvider([tool_turn, text_reply("Sorry.")])

    out = run(provider, "hi")

    assert "[tool] ask_user(" in out
    assert "[tool error] " in out and "2-6 choices" in out
    assert out.endswith("Sorry.\n")


def test_main_with_fake_and_a_situation_file(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(cli.config, "APP_DIR", tmp_path)  # no .env there
    situation = tmp_path / "situation.json"
    situation.write_text(json.dumps({"hero": "Dooley"}), encoding="utf-8")

    cli.main(["-m", "hi", "--fake", "--situation", str(situation)])

    assert "You asked: hi" in capsys.readouterr().out


def test_main_says_what_is_wrong(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    with pytest.raises(SystemExit, match="not JSON"):
        bad = tmp_path / "bad.json"
        bad.write_text("{", encoding="utf-8")
        cli.main(["-m", "hi", "--fake", "--situation", str(bad)])

    # OpenRouter without a key: a message, not a traceback.
    monkeypatch.setattr(cli.config, "APP_DIR", tmp_path)  # no .env there
    for key in ("ADVISOR_BASE_URL", "ADVISOR_API_KEY", "OPENROUTER_API_KEY"):
        monkeypatch.delenv(key, raising=False)
    with pytest.raises(SystemExit, match="No API key for OpenRouter"):
        cli.main(["-m", "hi"])
