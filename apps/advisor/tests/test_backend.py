"""The FastAPI service with fake providers: health, Conversations by Run, a reply's events in order,
saving and stats, one reply at a time, cancelling by closing the stream, and history.db's version.

Most tests use FastAPI's test client. Cancelling needs a real connection to close, so those tests run
the app in uvicorn on a free port of 127.0.0.1."""

from __future__ import annotations

import asyncio
import json
import sqlite3
import threading
import time
from collections.abc import AsyncIterator, Iterator
from pathlib import Path
from typing import Any

import httpx
import pytest
import uvicorn
from conftest import make_history_db
from fastapi.testclient import TestClient
from scraped_runs import CATALOG, SITUATION
from tau_agent.messages import AssistantMessage, Usage, UsageCost
from tau_agent.provider_events import (
    AssistantDoneEvent,
    AssistantStartEvent,
    TextDeltaEvent,
    ThinkingDeltaEvent,
    ToolCallDeltaEvent,
)
from tau_ai import FakeProvider

from agents.core.fake import advisor_turn, text_reply
from backend.db.store import TITLE_MAX_LENGTH, title_from
from backend.main import HOST, create_app
from backend.routers.health import VERSION
from backend.service import Service


# The fields of a reply's stats and of a step's: the `stats` event keeps this shape.
STATS_FIELDS = {"model", "elapsedMs", "tokens", "costUsd", "pricingSource", "toolCalls"}


def make_client(history_db: Path, provider_for, catalog=CATALOG) -> TestClient:
    return TestClient(create_app(Service(history_db, provider_for, "fake", catalog=catalog)))


def reply(text: str = "Take the Cutlass."):
    """A provider_for that answers every message with ``text``."""
    return lambda _text, _situation: FakeProvider([text_reply(text)])


def sse_events(body: str) -> list[dict[str, Any]]:
    """The events in an SSE body, checking each frame's event name is its type."""
    events = []
    for frame in body.strip().split("\n\n"):
        lines = frame.split("\n")
        assert lines[0].startswith("event: ") and lines[1].startswith("data: "), frame
        event = json.loads(lines[1].removeprefix("data: "))
        assert lines[0].removeprefix("event: ") == event["type"]
        events.append(event)
    return events


def summary(events: list[dict[str, Any]]) -> list[str]:
    """Each event as `type`, `type:role` for messages, `delta:kind` for deltas."""
    out = []
    for event in events:
        if event["type"] in ("message_start", "message_end"):
            out.append(f"{event['type']}:{event['message']['role']}")
        elif event["type"] == "delta":
            out.append(f"delta:{event['kind']}")
        else:
            out.append(event["type"])
    return out


def start(client: TestClient, run_id: str = "run-1", text: str = "Which?", situation=None) -> dict[str, Any]:
    response = client.post(f"/api/runs/{run_id}/conversations", json={"text": text, "situation": situation})
    assert response.status_code == 201, response.text
    return response.json()


def send(client: TestClient, conversation_id: str, text: str = "Which?", situation=None) -> list[dict[str, Any]]:
    response = client.post(
        f"/api/conversations/{conversation_id}/messages", json={"text": text, "situation": situation}
    )
    assert response.status_code == 200, response.text
    assert response.headers["content-type"].startswith("text/event-stream")
    return sse_events(response.text)


# Health and history.db's version


def test_health_gives_the_versions(history_db: Path) -> None:
    with make_client(history_db, reply()) as client:
        body = client.get("/api/health").json()
    assert body["ok"] is True
    assert body["version"] == VERSION
    assert body["historyDb"]["version"] == 9
    assert body["historyDb"]["expected"] == 9
    assert body["historyDb"]["error"] is None


def test_a_wrong_user_version_is_refused_and_left_untouched(tmp_path: Path) -> None:
    path = make_history_db(tmp_path / "history.db", 8)
    with make_client(path, reply()) as client:
        health = client.get("/api/health").json()
        refused = client.get("/api/runs/run-1/conversations")
        refused_post = client.post("/api/runs/run-1/conversations", json={"text": "Which?"})

    assert health["ok"] is False
    assert health["historyDb"]["version"] == 8
    assert "schema version 8, but the Advisor is written for version 9" in health["historyDb"]["error"]
    assert refused.status_code == 503
    assert "schema version 8" in refused.json()["detail"]
    assert "just tracker run" in refused.json()["detail"]
    assert refused_post.status_code == 503
    raw = sqlite3.connect(path)
    assert raw.execute("PRAGMA user_version").fetchone()[0] == 8
    raw.close()


def test_history_db_is_used_once_the_tracker_migrated_it(tmp_path: Path) -> None:
    path = make_history_db(tmp_path / "history.db", 8)
    with make_client(path, reply()) as client:
        assert client.get("/api/runs/run-1/conversations").status_code == 503
        path.unlink()
        make_history_db(path, 9)
        assert client.get("/api/health").json()["ok"] is True
        assert client.get("/api/runs/run-1/conversations").status_code == 404


def test_a_missing_history_db_is_reported_not_created(tmp_path: Path) -> None:
    path = tmp_path / "history.db"
    with make_client(path, reply()) as client:
        health = client.get("/api/health").json()
        assert client.get("/api/runs/run-1/conversations").status_code == 503
    assert health["ok"] is False
    assert health["historyDb"]["version"] is None
    assert "No history.db" in health["historyDb"]["error"]
    assert not path.exists()


# Conversations


def test_a_new_conversation_keeps_its_run_title_day_and_hour(history_db: Path) -> None:
    question = "Which of these three items should I take for my Jules board with the lollipop?"
    with make_client(history_db, reply()) as client:
        created = start(client, text=question, situation={"hero": "Jules", "day": 5, "hour": 2})
        listed = client.get("/api/runs/run-1/conversations").json()

    assert created["runId"] == "run-1"
    assert created["title"] == title_from(question)
    assert created["title"].endswith("…") and len(created["title"]) <= TITLE_MAX_LENGTH
    assert (created["day"], created["hour"]) == (5, 2)
    assert listed == [created]
    raw = sqlite3.connect(history_db)
    assert raw.execute("SELECT run_id, title, day, hour FROM advisor_conversations").fetchone() == (
        "run-1",
        created["title"],
        5,
        2,
    )
    raw.close()


def test_titles() -> None:
    assert title_from("  Which   one?\n") == "Which one?"
    long = "word " * 30
    assert title_from(long) == ("word " * 9).strip() + "…"


def test_conversations_are_listed_newest_first(history_db: Path) -> None:
    with make_client(history_db, reply()) as client:
        first = start(client, text="First")
        time.sleep(0.005)
        second = start(client, text="Second")
        assert [c["id"] for c in client.get("/api/runs/run-1/conversations").json()] == [second["id"], first["id"]]
        assert first["day"] is None and first["hour"] is None


def test_an_unknown_run_or_conversation_is_404(history_db: Path) -> None:
    with make_client(history_db, reply()) as client:
        assert client.get("/api/runs/no-such-run/conversations").status_code == 404
        assert client.post("/api/runs/no-such-run/conversations", json={"text": "Which?"}).status_code == 404
        assert client.get("/api/conversations/nope/messages").status_code == 404
        assert client.post("/api/conversations/nope/messages", json={"text": "Which?"}).status_code == 404
        assert client.delete("/api/conversations/nope").status_code == 404
        assert client.get("/api/runs/no-such-run/conversations").json()["detail"] == "Run not found."


def test_bad_bodies_are_400(history_db: Path) -> None:
    with make_client(history_db, reply()) as client:
        assert client.post("/api/runs/run-1/conversations", json={"text": "  "}).status_code == 400
        bad = client.post("/api/runs/run-1/conversations", json={"text": "Which?", "situation": {"dya": 5}})
        assert bad.status_code == 400
        assert "dya" in bad.json()["detail"]
        conversation = start(client)
        url = f"/api/conversations/{conversation['id']}/messages"
        assert client.post(url, json={"text": ""}).status_code == 400
        bad = client.post(url, json={"text": "Which?", "situation": {"offer": [{"name": "Cutlass"}]}})
        assert bad.status_code == 400
        assert "offer.0.kind" in bad.json()["detail"]
        assert client.get(url).json()["messages"] == []


def test_deleting_a_conversation_deletes_its_messages(history_db: Path) -> None:
    with make_client(history_db, reply()) as client:
        conversation = start(client)
        send(client, conversation["id"])
        assert client.delete(f"/api/conversations/{conversation['id']}").status_code == 204
        assert client.get("/api/runs/run-1/conversations").json() == []
    raw = sqlite3.connect(history_db)
    assert raw.execute("SELECT COUNT(*) FROM advisor_messages").fetchone() == (0,)
    raw.close()


# A reply


def test_a_reply_streams_its_events_in_order_and_is_saved_with_stats(history_db: Path) -> None:
    with make_client(history_db, reply("Take the Cutlass.")) as client:
        conversation = start(client)
        events = send(client, conversation["id"])
        stored = client.get(f"/api/conversations/{conversation['id']}/messages").json()

    assert summary(events) == [
        "message_start:user",
        "message_end:user",
        "message_start:assistant",
        "delta:text",
        "message_end:assistant",
        "step",
        "stats",
        "done",
    ]
    assert events[0]["message"]["content"] == "Which?"
    assert events[3] == {"type": "delta", "kind": "text", "index": 0, "delta": "Take the Cutlass."}
    step = events[5]
    assert step["seq"] == 1
    stats = events[6]
    assert stats["seq"] == 1
    assert stats["stats"]["model"] == "fake"
    assert stats["stats"]["toolCalls"] == 0
    assert set(stats["stats"]) == STATS_FIELDS
    assert set(step["step"]) == STATS_FIELDS

    assert stored["running"] is False
    assert [m["seq"] for m in stored["messages"]] == [0, 1]
    assert [m["message"]["role"] for m in stored["messages"]] == ["user", "assistant"]
    assert stored["messages"][0]["stats"] is None and stored["messages"][0]["step"] is None
    assert stored["messages"][1]["stats"] == stats["stats"]
    assert stored["messages"][1]["step"] == step["step"]
    assert stored["messages"][1]["message"]["content"][0]["text"] == "Take the Cutlass."
    raw = sqlite3.connect(history_db)
    rows = raw.execute("SELECT seq, role, stats IS NOT NULL FROM advisor_messages ORDER BY seq").fetchall()
    raw.close()
    assert rows == [(0, "user", 0), (1, "assistant", 1)]


def test_the_next_message_carries_the_history(history_db: Path) -> None:
    providers: list[FakeProvider] = []

    def provider_for(text: str, _situation: object) -> FakeProvider:
        providers.append(FakeProvider([text_reply(f"Answer to {text}")]))
        return providers[-1]

    with make_client(history_db, provider_for) as client:
        conversation = start(client)
        send(client, conversation["id"], "First?")
        events = send(client, conversation["id"], "Second?")
    assert events[-2]["seq"] == 3
    messages = providers[1].calls[0][2]
    assert [m.role for m in messages] == ["user", "assistant", "user"]


def test_delta_kinds_and_usage(history_db: Path) -> None:
    usage = Usage(input=100, output=20, cache_read=5, total_tokens=125, cost=UsageCost(total=0.0012))
    stream = [
        AssistantStartEvent(partial=AssistantMessage(model="fake")),
        ThinkingDeltaEvent(content_index=0, delta="Hmm.", partial=AssistantMessage()),
        ToolCallDeltaEvent(content_index=1, delta='{"q', partial=AssistantMessage()),
        TextDeltaEvent(content_index=2, delta="Done.", partial=AssistantMessage(content="Done.")),
        AssistantDoneEvent(
            reason="stop", message=AssistantMessage(content="Done.", model="fake", stop_reason="stop", usage=usage)
        ),
    ]
    with make_client(history_db, lambda _t, _s: FakeProvider([stream])) as client:
        events = send(client, start(client)["id"])
    deltas = [(e["kind"], e["index"], e["delta"]) for e in events if e["type"] == "delta"]
    assert deltas == [("thinking", 0, "Hmm."), ("toolcall", 1, '{"q'), ("text", 2, "Done.")]
    stats = next(e for e in events if e["type"] == "stats")["stats"]
    assert stats["tokens"] == {"input": 100, "output": 20, "cacheRead": 5, "cacheWrite": 0, "total": 125}
    assert stats["costUsd"] == pytest.approx(0.0012)


class SlowSteps:
    """Answers each model call with the next of ``replies`` after ``delay`` seconds."""

    def __init__(self, replies: list[list[object]], delay: float) -> None:
        self._replies = list(replies)
        self._delay = delay

    def stream_response(self, **_kwargs) -> AsyncIterator[Any]:
        events = self._replies.pop(0)

        async def iterator() -> AsyncIterator[Any]:
            await asyncio.sleep(self._delay)
            for event in events:
                yield event

        return iterator()


def step_reply(content, *, stop: str, model: str, usage: Usage) -> list[object]:
    message = AssistantMessage(content=content, model="fake", response_model=model, stop_reason=stop, usage=usage)
    return [AssistantStartEvent(partial=AssistantMessage(model="fake")), AssistantDoneEvent(reason=stop, message=message)]


def test_a_two_step_reply_saves_each_steps_stats_and_their_total(history_db: Path) -> None:
    from tau_agent.messages import ToolCall

    first = Usage(input=1000, output=50, cache_read=200, cache_write=10, total_tokens=1260, cost=UsageCost(total=0.002))
    second = Usage(input=1300, output=80, cache_read=900, cache_write=0, total_tokens=2280, cost=UsageCost(total=0.003))
    call = ToolCall(id="call-1", name="find_card", arguments={"query": "Cutlass"})
    provider = SlowSteps(
        [
            step_reply([call], stop="toolUse", model="vendor/model-a", usage=first),
            step_reply("Take the Cutlass.", stop="stop", model="vendor/model-b", usage=second),
        ],
        delay=0.05,
    )
    with make_client(history_db, lambda _t, _s: provider) as client:
        conversation = start(client)
        events = send(client, conversation["id"])
        stored = client.get(f"/api/conversations/{conversation['id']}/messages").json()["messages"]

    assert [e for e in summary(events) if not e.startswith("delta")] == [
        "message_start:user",
        "message_end:user",
        "message_start:assistant",
        "message_end:assistant",
        "step",
        "message_start:toolResult",
        "message_end:toolResult",
        "message_start:assistant",
        "message_end:assistant",
        "step",
        "stats",
        "done",
    ]
    steps = [e for e in events if e["type"] == "step"]
    total = next(e for e in events if e["type"] == "stats")
    assert [s["seq"] for s in steps] == [1, 3] and total["seq"] == 3
    a, b = (s["step"] for s in steps)
    assert (a["model"], b["model"]) == ("vendor/model-a", "vendor/model-b")
    assert a["tokens"] == {"input": 1000, "output": 50, "cacheRead": 200, "cacheWrite": 10, "total": 1260}
    assert b["tokens"] == {"input": 1300, "output": 80, "cacheRead": 900, "cacheWrite": 0, "total": 2280}
    assert (a["costUsd"], b["costUsd"]) == (pytest.approx(0.002), pytest.approx(0.003))
    assert (a["toolCalls"], b["toolCalls"]) == (1, 0)
    # Each step's time is its model call's; the reply's covers both and the tool call between.
    assert a["elapsedMs"] >= 40 and b["elapsedMs"] >= 40
    assert total["stats"]["elapsedMs"] >= a["elapsedMs"] + b["elapsedMs"]

    # The reply's total: its shape unchanged, its tokens and cost the steps' sum.
    assert set(total["stats"]) == STATS_FIELDS
    assert total["stats"]["model"] == "fake"
    for key in ("input", "output", "cacheRead", "cacheWrite", "total"):
        assert total["stats"]["tokens"][key] == a["tokens"][key] + b["tokens"][key]
    assert total["stats"]["costUsd"] == pytest.approx(a["costUsd"] + b["costUsd"])
    assert total["stats"]["toolCalls"] == 1

    # A reloaded Conversation has every step, and the total on the last message only.
    assert [m["message"]["role"] for m in stored] == ["user", "assistant", "toolResult", "assistant"]
    assert [m["step"] for m in stored] == [None, a, None, b]
    assert [m["stats"] for m in stored] == [None, None, None, total["stats"]]

    # No schema change: both live in the stats column, the step under "step".
    raw = sqlite3.connect(history_db)
    columns = [json.loads(c) if c else None for (c,) in raw.execute("SELECT stats FROM advisor_messages ORDER BY seq")]
    raw.close()
    assert columns == [None, {"step": a}, None, {**total["stats"], "step": b}]


def test_a_row_saved_before_step_stats_reads_as_the_total() -> None:
    from backend.db.store import split_stats

    old = {"model": "m", "elapsedMs": 5, "tokens": {}, "costUsd": 0, "pricingSource": "provider", "toolCalls": 0}
    assert split_stats(old) == (old, None)
    assert split_stats({"step": old}) == (None, old)
    assert split_stats(None) == (None, None)


def test_a_fake_advisor_turn_runs_the_tools_and_saves_every_message(history_db: Path, sources) -> None:
    """``sources`` imports the record's example into ``history_db``: the tools really count."""
    with make_client(history_db, advisor_turn) as client:
        conversation = start(client, situation=SITUATION)
        events = send(client, conversation["id"], "Which of these should I take?", SITUATION)
        stored = client.get(f"/api/conversations/{conversation['id']}/messages").json()["messages"]

    roles = [e["message"]["role"] for e in events if e["type"] == "message_end"]
    assert roles == [
        "user",
        "assistant",
        "toolResult",
        "assistant",
        "toolResult",
        "assistant",
        "toolResult",
        "assistant",
    ]
    tool_results = [e["message"] for e in events if e["type"] == "message_end" and e["message"]["role"] == "toolResult"]
    assert [r["toolName"] for r in tool_results] == ["rate_options", "suggest", "follow_ups"]
    assert not any(r.get("isError") for r in tool_results)
    # The app draws the recommendation card from the result's details, source runs included.
    card = tool_results[1]["details"]
    assert card["n"] == 15 and card["runs"]
    stats = next(e for e in events if e["type"] == "stats")
    assert stats["stats"]["toolCalls"] == 3
    assert summary(events)[-2:] == ["stats", "done"]
    assert [m["message"]["role"] for m in stored] == roles
    assert [m["seq"] for m in stored if m["stats"]] == [stats["seq"]] == [len(stored) - 1]


def test_ask_user_ends_the_reply(history_db: Path) -> None:
    from agents.core.fake import tool_reply

    arguments = {"question": "Which hero?", "options": [{"id": "a", "label": "Jules"}, {"id": "b", "label": "Vanessa"}]}
    provider = FakeProvider([tool_reply("ask_user", arguments), text_reply("never asked for")])
    with make_client(history_db, lambda _t, _s: provider) as client:
        events = send(client, start(client)["id"])
    roles = [e["message"]["role"] for e in events if e["type"] == "message_end"]
    assert roles == ["user", "assistant", "toolResult"]
    assert len(provider.calls) == 1
    assert summary(events)[-2:] == ["stats", "done"]


def test_a_failing_provider_sends_error_then_done(history_db: Path) -> None:
    class Broken:
        def stream_response(self, **_kwargs):
            raise RuntimeError("the model is down")

    with make_client(history_db, lambda _t, _s: Broken()) as client:
        conversation = start(client)
        events = send(client, conversation["id"])
        assert client.get(f"/api/conversations/{conversation['id']}/messages").json()["running"] is False
    assert summary(events)[-1] == "done"
    errors = [e for e in events if e["type"] == "error"] + [
        e for e in events if e["type"] == "message_end" and e["message"].get("errorMessage")
    ]
    assert errors, events


# One reply at a time, and cancelling: a real server, so the stream can be closed.


class BlockingProvider:
    """Streams "Partial" and then waits until the run is cancelled."""

    def __init__(self) -> None:
        self.cancelled = threading.Event()

    def stream_response(self, *, model, system, messages, tools, signal=None, session_id=None) -> AsyncIterator[Any]:
        async def iterator() -> AsyncIterator[Any]:
            yield AssistantStartEvent(partial=AssistantMessage(model="fake"))
            yield TextDeltaEvent(content_index=0, delta="Partial", partial=AssistantMessage(content="Partial"))
            try:
                while not (signal is not None and signal.is_cancelled()):
                    await asyncio.sleep(0.01)
            finally:
                self.cancelled.set()

        return iterator()


@pytest.fixture
def blocking_server(history_db: Path) -> Iterator[tuple[str, BlockingProvider]]:
    """The app in uvicorn on 127.0.0.1: "block" streams and waits, anything else answers at once."""
    blocking = BlockingProvider()

    def provider_for(text: str, _situation: object):
        return blocking if text == "block" else FakeProvider([text_reply("Quick.")])

    app = create_app(Service(history_db, provider_for, "fake", catalog=CATALOG))
    server = uvicorn.Server(uvicorn.Config(app, host=HOST, port=0, log_level="warning", lifespan="on"))
    thread = threading.Thread(target=server.run, daemon=True)
    thread.start()
    deadline = time.monotonic() + 10
    while not server.started:
        assert time.monotonic() < deadline, "the server didn't start"
        time.sleep(0.01)
    port = server.servers[0].sockets[0].getsockname()[1]
    try:
        yield f"http://{HOST}:{port}", blocking
    finally:
        server.should_exit = True
        thread.join(10)


def wait_until(check, timeout: float = 5.0) -> None:
    deadline = time.monotonic() + timeout
    while not check():
        assert time.monotonic() < deadline, "timed out"
        time.sleep(0.02)


def test_one_reply_at_a_time_and_closing_the_stream_cancels_it(blocking_server) -> None:
    base, blocking = blocking_server
    with httpx.Client(base_url=base, timeout=10) as client:
        conversation = client.post("/api/runs/run-1/conversations", json={"text": "block"}).json()
        url = f"/api/conversations/{conversation['id']}/messages"

        with client.stream("POST", url, json={"text": "block"}) as response:
            assert response.status_code == 200
            seen = []
            # Kept in a name: a dropped line iterator closes the response.
            lines = response.iter_lines()
            for line in lines:
                if line.startswith("data: "):
                    event = json.loads(line.removeprefix("data: "))
                    seen.append(event)
                    if event["type"] == "delta":
                        break
            assert summary(seen) == ["message_start:user", "message_end:user", "message_start:assistant", "delta:text"]

            # While it streams: a second message and deleting are refused, and the list says running.
            second = client.post(url, json={"text": "Another?"})
            assert second.status_code == 409, second.text
            assert second.json()["detail"] == "A reply is still running."
            assert client.delete(f"/api/conversations/{conversation['id']}").status_code == 409
            assert client.get(url).json()["running"] is True

        # Closing the stream cancels the reply and keeps what arrived.
        assert blocking.cancelled.wait(5)
        wait_until(lambda: client.get(url).json()["running"] is False)
        stored = client.get(url).json()["messages"]
        assert [m["message"]["role"] for m in stored] == ["user", "assistant"]
        stopped = stored[1]["message"]
        assert stopped["stopReason"] == "aborted"
        assert stopped["content"] == [{"type": "text", "text": "Partial"}]
        assert stored[1]["stats"] is None

        # The Conversation can be asked again; the stopped reply is in its history.
        events = sse_events(client.post(url, json={"text": "Again?"}).text)
        assert summary(events)[-2:] == ["stats", "done"]
        assert [m["seq"] for m in client.get(url).json()["messages"]] == [0, 1, 2, 3]
