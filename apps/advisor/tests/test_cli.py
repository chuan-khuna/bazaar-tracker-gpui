"""The terminal runner with a fake provider: the reply, tool lines, the situation above the question, and a
run that calls rate_options, suggest and follow_ups on the record's example."""

from __future__ import annotations

import asyncio
import io
import json
from pathlib import Path

import pytest
from tau_agent.messages import AssistantMessage, ToolCall
from tau_agent.provider_events import AssistantDoneEvent, AssistantStartEvent, ToolCallEndEvent
from tau_ai import FakeProvider

from scraped_runs import SITUATION, write_record_example
from tau_agent import ToolResultMessage

from agents import AGENTS
from agents.advisor.sources import Sources
from agents.advisor.tools import TOOL_NAMES
from agents.core import cli
from agents.core.fake import advisor_turn, fake_provider, text_reply, tool_reply
from conftest import make_history_db


def run(
    provider: FakeProvider, message: str, situation: object | None = None, sources: Sources | None = None
) -> str:
    out = io.StringIO()
    asyncio.run(
        cli.run(
            AGENTS["advisor"], message, provider=provider, model="fake", situation=situation, sources=sources, out=out
        )
    )
    return out.getvalue()


def test_prints_the_reply() -> None:
    provider = fake_provider("Take the Cutlass.")
    assert run(provider, "hi") == "Take the Cutlass.\n"
    _model, _system, messages, tools = provider.calls[0]
    assert messages[0].role == "user" and "The app sent no situation" in messages[0].text
    assert tuple(tool.name for tool in tools) == TOOL_NAMES


def test_puts_the_situation_above_the_question() -> None:
    provider = fake_provider("ok")
    run(provider, "Which?", situation={"hero": "Vanessa", "day": 5, "offer": [{"name": "Cutlass", "kind": "loot"}]})
    _model, system, messages, _tools = provider.calls[0]
    assert "Hero: Vanessa" not in system
    question = messages[0].text
    assert "## Situation for this question" in question
    assert "Hero: Vanessa · Day 5" in question
    assert "- Cutlass (loot)" in question
    assert question.endswith("## Question\n\nWhich?")


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


def test_a_fake_run_that_calls_suggest(sources: Sources) -> None:
    ranked = [
        {"card": "Laurel's Bouncy Ball", "reason": "Shows up with both your items"},
        {"card": "Decanter", "reason": "Close behind with Pizza Cutter"},
        {"card": "Cheese Wheel", "reason": "Rarely with your items"},
    ]
    provider = FakeProvider(
        [
            tool_reply("rate_options", {}),
            tool_reply("suggest", {"title": "Shop · Day 5", "ranked": ranked}),
            tool_reply("follow_ups", {"questions": ["Where should I put it?", "Which skill fits?"]}),
            text_reply("Take Laurel's Bouncy Ball."),
        ]
    )

    out = run(provider, "Which of these should I take?", situation=SITUATION, sources=sources)

    assert "[tool] rate_options({})" in out and "[tool] suggest(" in out and "[tool] follow_ups(" in out
    assert "[tool error]" not in out
    assert out.endswith("Take Laurel's Bouncy Ball.\n")
    # The model went on after suggest and follow_ups, and read suggest's numbers without the source runs.
    assert len(provider.calls) == 4
    results = [m for m in provider.calls[3][2] if isinstance(m, ToolResultMessage)]
    assert [r.tool_name for r in results] == ["rate_options", "suggest", "follow_ups"]
    suggested = json.loads(results[1].content[0].text)
    assert suggested["source"] == "15 Jules runs · S20 · 1 profile"
    assert [row["card"] for row in suggested["ranked"]] == ["Laurel's Bouncy Ball", "Decanter", "Cheese Wheel"]
    assert "runs" not in suggested
    assert results[1].details["runs"]["jules-01"]["url"].endswith("/jules-01")


def test_the_fake_flag_runs_the_tools_on_the_offer(sources: Sources) -> None:
    out = run(advisor_turn("Which?", SITUATION), "Which?", situation=SITUATION, sources=sources)
    assert "[tool] rate_options" in out and "[tool] suggest" in out and "[tool] follow_ups" in out
    assert "[tool error]" not in out
    assert out.endswith("(fake provider, no model called) You asked: Which?\n")
    # Without an Offer, text alone.
    assert run(advisor_turn("hi", {"hero": "Jules"}), "hi", sources=sources).startswith("(fake provider")


def test_main_with_fake_reads_the_data_folder(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], monkeypatch: pytest.MonkeyPatch
) -> None:
    from backend.db import open_history
    from importer import import_builds

    make_history_db(tmp_path / "history.db", 9)
    write_record_example(tmp_path / "builds")
    engine = open_history(tmp_path / "history.db")
    import_builds(engine, tmp_path / "builds")
    engine.dispose()
    monkeypatch.setattr(cli.config, "APP_DIR", tmp_path)  # no .env there
    monkeypatch.setenv("BAZAAR_TRACKER_DATA_DIR", str(tmp_path))
    # No game folder: the card catalog is empty, whatever this machine has installed.
    monkeypatch.setenv("USERPROFILE", str(tmp_path))
    monkeypatch.setenv("HOME", str(tmp_path))
    situation = tmp_path / "situation.json"
    situation.write_text(json.dumps(SITUATION), encoding="utf-8")

    cli.main(["-m", "Which?", "--fake", "--situation", str(situation)])

    out = capsys.readouterr().out
    # No GameData.db fixture here, so the Offer's cards are known by their ids in the situation.
    assert "[tool] suggest(" in out and "[tool error]" not in out


def test_main_says_what_is_wrong(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    with pytest.raises(SystemExit, match="not JSON"):
        bad = tmp_path / "bad.json"
        bad.write_text("{", encoding="utf-8")
        cli.main(["-m", "hi", "--fake", "--situation", str(bad)])
    with pytest.raises(SystemExit, match="offer.0.kind: Field required"):
        invalid = tmp_path / "invalid.json"
        invalid.write_text(json.dumps({"offer": [{"name": "A"}]}), encoding="utf-8")
        cli.main(["-m", "hi", "--fake", "--situation", str(invalid)])

    # OpenRouter without a key: a message, not a traceback.
    monkeypatch.setattr(cli.config, "APP_DIR", tmp_path)  # no .env there
    for key in ("ADVISOR_BASE_URL", "ADVISOR_API_KEY", "OPENROUTER_API_KEY"):
        monkeypatch.delenv(key, raising=False)
    with pytest.raises(SystemExit, match="No API key for OpenRouter"):
        cli.main(["-m", "hi"])
