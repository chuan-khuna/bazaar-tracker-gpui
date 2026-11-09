"""Choosing the model: a message's ``model`` (used, defaulted, 400) and `GET /api/models` (fetched,
cached, fallback), with a fake HTTP layer (httpx.MockTransport): no real request is made."""

from __future__ import annotations

import json
from pathlib import Path

import httpx
from fastapi.testclient import TestClient
from scraped_runs import CATALOG
from tau_ai import FakeProvider
from test_backend import sse_events, start

from agents.core.fake import text_reply
from backend.main import create_app
from backend.models import ModelCatalog
from backend.service import Service

DEFAULT = "vendor/default-model"
KEY = "sk-or-test-key"


def client_with(history_db: Path, provider: FakeProvider, models: ModelCatalog | None = None) -> TestClient:
    service = Service(history_db, lambda _t, _s: provider, DEFAULT, catalog=CATALOG)
    return TestClient(create_app(service, models=models))


def unnamed_reply(text: str = "Take the Cutlass.") -> list[object]:
    """A reply whose message names no model, as when the provider doesn't say: the step falls back
    to the model the reply was asked of."""
    return text_reply(text, model="unknown")


# A message's model


def test_a_given_model_answers_the_message_and_is_in_its_stats(history_db: Path) -> None:
    provider = FakeProvider([unnamed_reply()])
    with client_with(history_db, provider) as client:
        conversation = start(client)
        response = client.post(
            f"/api/conversations/{conversation['id']}/messages", json={"text": "Which?", "model": " vendor/other:free "}
        )
    assert response.status_code == 200, response.text
    events = sse_events(response.text)
    assert [call[0] for call in provider.calls] == ["vendor/other:free"]
    step = next(e for e in events if e["type"] == "step")
    stats = next(e for e in events if e["type"] == "stats")
    assert step["step"]["model"] == "vendor/other:free"
    assert stats["stats"]["model"] == "vendor/other:free"


def test_without_a_model_the_default_answers(history_db: Path) -> None:
    provider = FakeProvider([unnamed_reply(), unnamed_reply()])
    with client_with(history_db, provider) as client:
        conversation = start(client)
        url = f"/api/conversations/{conversation['id']}/messages"
        first = sse_events(client.post(url, json={"text": "Which?"}).text)
        second = sse_events(client.post(url, json={"text": "And now?", "model": None}).text)
    assert [call[0] for call in provider.calls] == [DEFAULT, DEFAULT]
    for events in (first, second):
        assert next(e for e in events if e["type"] == "stats")["stats"]["model"] == DEFAULT


def test_a_blank_or_malformed_model_is_400(history_db: Path) -> None:
    provider = FakeProvider([])
    with client_with(history_db, provider) as client:
        conversation = start(client)
        url = f"/api/conversations/{conversation['id']}/messages"
        for model in ("", "   ", "has space", "-leading", "a" * 201, 5, ["x"]):
            response = client.post(url, json={"text": "Which?", "model": model})
            assert response.status_code == 400, (model, response.text)
        assert client.get(url).json()["messages"] == []
    assert provider.calls == []


# GET /api/models


class FakeEndpoint:
    """An OpenAI-compatible `/models` endpoint: counts requests, keeps the headers, can fail."""

    def __init__(self, ids: list[str], *, status: int = 200) -> None:
        self.ids = ids
        self.status = status
        self.requests: list[httpx.Request] = []

    def transport(self) -> httpx.MockTransport:
        def handle(request: httpx.Request) -> httpx.Response:
            self.requests.append(request)
            if self.status != 200:
                return httpx.Response(self.status, json={"error": "down"})
            data = [{"id": i, "name": i.upper()} for i in self.ids]
            return httpx.Response(200, content=json.dumps({"data": data}))

        return httpx.MockTransport(handle)


class Clock:
    def __init__(self) -> None:
        self.now = 1000.0

    def __call__(self) -> float:
        return self.now


def test_models_are_fetched_from_the_endpoint_with_the_default_first(history_db: Path) -> None:
    endpoint = FakeEndpoint(["z/model", "a/model"])
    catalog = ModelCatalog(DEFAULT, "https://openrouter.ai/api/v1/", KEY, transport=endpoint.transport())
    with client_with(history_db, FakeProvider([]), catalog) as client:
        body = client.get("/api/models").json()
    assert body["default"] == DEFAULT
    assert body["source"] == "endpoint"
    assert [m["id"] for m in body["models"]] == [DEFAULT, "a/model", "z/model"]
    assert body["models"][1] == {"id": "a/model", "name": "A/MODEL"}
    assert str(endpoint.requests[0].url) == "https://openrouter.ai/api/v1/models"
    assert endpoint.requests[0].headers["authorization"] == f"Bearer {KEY}"
    assert KEY not in repr(catalog)


def test_each_model_carries_its_input_and_output_price_per_million_tokens() -> None:
    data = [
        {"id": "a/priced", "name": "Priced", "pricing": {"prompt": "0.000003", "completion": "0.000015"}},
        {"id": "b/free", "name": "Free", "pricing": {"prompt": "0", "completion": "0"}},
        {"id": "c/router", "name": "Router", "pricing": {"prompt": "-1", "completion": "-1"}},
        {"id": "d/local", "name": "Local"},
    ]
    transport = httpx.MockTransport(lambda request: httpx.Response(200, json={"data": data}))
    models = {m["id"]: m for m in ModelCatalog("a/priced", "http://x/v1", None, transport=transport).to_json()["models"]}
    assert models["a/priced"] == {"id": "a/priced", "name": "Priced", "inputUsdPerMillion": 3.0, "outputUsdPerMillion": 15.0}
    assert models["b/free"] == {"id": "b/free", "name": "Free", "inputUsdPerMillion": 0.0, "outputUsdPerMillion": 0.0}
    assert models["c/router"] == {"id": "c/router", "name": "Router"}
    assert models["d/local"] == {"id": "d/local", "name": "Local"}

def test_the_list_is_cached_for_a_while(history_db: Path) -> None:
    endpoint = FakeEndpoint([DEFAULT, "a/model"])
    clock = Clock()
    catalog = ModelCatalog(DEFAULT, "http://localhost:1234/v1", None, ttl=60, transport=endpoint.transport(), clock=clock)
    with client_with(history_db, FakeProvider([]), catalog) as client:
        first = client.get("/api/models").json()
        clock.now += 59
        assert client.get("/api/models").json() == first
        assert len(endpoint.requests) == 1
        endpoint.ids = [DEFAULT, "b/model"]
        clock.now += 2
        later = client.get("/api/models").json()
    assert len(endpoint.requests) == 2
    assert [m["id"] for m in first["models"]] == ["a/model", DEFAULT]
    assert [m["id"] for m in later["models"]] == ["b/model", DEFAULT]
    # No key: no Authorization header (a local model).
    assert "authorization" not in endpoint.requests[0].headers


def test_a_failed_fetch_still_gives_the_default(history_db: Path) -> None:
    endpoint = FakeEndpoint(["a/model"], status=500)
    catalog = ModelCatalog(DEFAULT, "http://localhost:1234/v1", None, transport=endpoint.transport())
    with client_with(history_db, FakeProvider([]), catalog) as client:
        failed = client.get("/api/models")
        # A failure isn't cached: the next request asks again.
        endpoint.status = 200
        fetched = client.get("/api/models").json()
    assert failed.status_code == 200
    assert failed.json() == {"default": DEFAULT, "models": [{"id": DEFAULT, "name": DEFAULT}], "source": "default"}
    assert fetched["source"] == "endpoint"
    assert len(endpoint.requests) == 2


def test_an_unreachable_or_odd_endpoint_gives_the_default(history_db: Path) -> None:
    def refuse(request: httpx.Request) -> httpx.Response:
        raise httpx.ConnectError("refused", request=request)

    def odd(_request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, content=b"not json")

    for transport in (httpx.MockTransport(refuse), httpx.MockTransport(odd)):
        catalog = ModelCatalog(DEFAULT, "http://localhost:1234/v1", None, transport=transport)
        assert catalog.to_json()["source"] == "default"


def test_without_an_endpoint_the_list_is_the_default_alone(history_db: Path) -> None:
    with client_with(history_db, FakeProvider([])) as client:
        body = client.get("/api/models").json()
    assert body == {"default": DEFAULT, "models": [{"id": DEFAULT, "name": DEFAULT}], "source": "default"}
