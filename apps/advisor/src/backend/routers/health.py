"""`GET /api/health`: the Advisor's version and history.db's."""

from __future__ import annotations

import importlib.metadata
from typing import Any

from fastapi import APIRouter

from backend.deps import ServiceDep

router = APIRouter(tags=["health"])

VERSION = importlib.metadata.version("bazaar-advisor")


@router.get("/health")
def health(service: ServiceDep) -> dict[str, Any]:
    """Always 200 while the service runs; ``ok`` is false while history.db can't be used."""
    status = service.history_status()
    return {"ok": status["error"] is None, "version": VERSION, "historyDb": status}
