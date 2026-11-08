"""`GET /api/models`: the default model and the list the app's model picker offers."""

from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Request

router = APIRouter(tags=["models"])


@router.get("/models")
def list_models(request: Request) -> dict[str, Any]:
    """Always 200: when the endpoint's list can't be fetched, ``models`` is the default alone."""
    return request.app.state.models.to_json()
