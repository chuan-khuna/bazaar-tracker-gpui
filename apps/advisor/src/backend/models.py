"""The models the app can choose from: `GET /api/models`, and checking a message's ``model``.

The list is the endpoint's OpenAI-compatible ``GET {base_url}/models`` (OpenRouter's, or a local
model's), cached for ``ttl`` seconds after a fetch that worked. When the fetch fails, the last list
fetched is kept, else the list is just the default model, as it is with no endpoint to ask
(``--fake``); a failure isn't cached, so the next request tries again. The key, when there is one, goes in the Authorization header and nowhere else.
"""

from __future__ import annotations

import logging
import re
import time
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

import httpx
from fastapi import HTTPException

log = logging.getLogger(__name__)

# How long a fetched list is kept.
MODELS_TTL_SECONDS = 600.0
# How long to wait for the endpoint's list.
FETCH_TIMEOUT_SECONDS = 10.0
# A model id as OpenRouter and local servers write them: `vendor/name:tag`, `llama3.1:8b`, ...
_MODEL_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:/@+\-]*$")
MODEL_MAX_LENGTH = 200


def checked_model(model: object) -> str | None:
    """A message's ``model``: None when not given; 400 when blank or not a model id."""
    if model is None:
        return None
    if not isinstance(model, str) or not model.strip():
        raise HTTPException(400, "model must be a model id, such as vendor/name.")
    model = model.strip()
    if len(model) > MODEL_MAX_LENGTH or not _MODEL_ID.match(model):
        raise HTTPException(400, f"model {model[:MODEL_MAX_LENGTH]!r} is not a model id.")
    return model


@dataclass(frozen=True, slots=True)
class ModelInfo:
    id: str
    name: str

    def to_json(self) -> dict[str, str]:
        return {"id": self.id, "name": self.name}


class ModelCatalog:
    """The default model and the endpoint's list. ``base_url`` None: no endpoint, the default alone.
    ``transport``: an httpx transport for tests (no real request)."""

    def __init__(
        self,
        default: str,
        base_url: str | None = None,
        api_key: str | None = None,
        *,
        ttl: float = MODELS_TTL_SECONDS,
        transport: httpx.BaseTransport | None = None,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self.default = default
        self._base_url = base_url.rstrip("/") if base_url else None
        self._api_key = api_key
        self._ttl = ttl
        self._transport = transport
        self._clock = clock
        self._cached: list[ModelInfo] | None = None
        self._fetched_at = 0.0

    def __repr__(self) -> str:
        key = "set" if self._api_key else "None"
        return f"ModelCatalog(default={self.default!r}, base_url={self._base_url!r}, api_key={key})"

    def to_json(self) -> dict[str, Any]:
        """``default``, ``models`` (the default first when the endpoint doesn't list it) and ``source``:
        "endpoint" for a fetched (or cached) list, "default" when only the default is known."""
        fetched = self.models()
        if fetched is None:
            models, source = [ModelInfo(self.default, self.default)], "default"
        else:
            models, source = fetched, "endpoint"
            if not any(m.id == self.default for m in models):
                models = [ModelInfo(self.default, self.default), *models]
        return {"default": self.default, "models": [m.to_json() for m in models], "source": source}

    def models(self) -> list[ModelInfo] | None:
        """The endpoint's list, from the cache while it is fresh; None when it can't be had."""
        if self._base_url is None:
            return None
        now = self._clock()
        if self._cached is not None and now - self._fetched_at < self._ttl:
            return self._cached
        try:
            models = self._fetch()
        except (httpx.HTTPError, ValueError, KeyError, TypeError) as error:
            log.warning("Couldn't fetch the model list from %s/models: %s", self._base_url, error)
            return self._cached
        self._cached = models
        self._fetched_at = now
        return models

    def _fetch(self) -> list[ModelInfo]:
        headers = {"Authorization": f"Bearer {self._api_key}"} if self._api_key else {}
        with httpx.Client(transport=self._transport, timeout=FETCH_TIMEOUT_SECONDS) as client:
            response = client.get(f"{self._base_url}/models", headers=headers)
            response.raise_for_status()
            body = response.json()
        models = []
        for entry in body["data"]:
            model_id = entry.get("id") if isinstance(entry, dict) else None
            if isinstance(model_id, str) and model_id:
                name = entry.get("name")
                models.append(ModelInfo(model_id, name if isinstance(name, str) and name else model_id))
        models.sort(key=lambda m: m.id)
        return models
