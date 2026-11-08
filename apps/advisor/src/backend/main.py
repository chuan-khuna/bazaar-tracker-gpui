"""The Advisor's FastAPI service, for the tracker's Advisor tab (built the way tau-app's backend is).

Layout:
- routers/     HTTP only, all under /api: health, conversations, messages
- schemas.py   request bodies, and checking the situation they carry
- deps.py      FastAPI dependencies (the Service, Store, Runs, the path's Conversation)
- service.py   history.db opened on first use, and the Store and Runs on it
- agent/       running the Advisor: runner (Runs), stream events, reply stats (no FastAPI)
- db/          opening history.db, the models, the Store

just advisor serve            # uvicorn on 127.0.0.1:ADVISOR_PORT (8420)
just advisor serve --fake     # no model: the fake provider, as `advisor run --fake`
"""

from __future__ import annotations

import argparse
import logging
from collections.abc import AsyncIterator, Sequence
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI

import config
from agents.core.fake import advisor_turn
from agents.core.provider import provider_from
from backend.agent.runner import ProviderFor
from backend.routers import conversations, health, messages
from backend.service import Service

# The service listens on this address only: the app on the same machine is its one client.
HOST = "127.0.0.1"

log = logging.getLogger(__name__)


def create_app(service: Service, *, on_close: Sequence[object] = ()) -> FastAPI:
    """The app around ``service``. ``on_close``: objects with an async ``aclose()`` (the provider)."""

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        status = service.history_status()
        if status["error"]:
            log.warning("history.db can't be used yet: %s", status["error"])
        try:
            yield
        finally:
            for resource in on_close:
                await resource.aclose()  # type: ignore[attr-defined]
            service.close()

    app = FastAPI(title="Bazaar Advisor", lifespan=lifespan)
    app.state.service = service
    for router in (health.router, conversations.router, messages.router):
        app.include_router(router, prefix="/api")
    return app


def main(argv: Sequence[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--fake", action="store_true", help="answer with the fake provider instead of a model")
    args = parser.parse_args(argv)

    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s: %(message)s")
    settings = config.load()
    on_close: list[object] = []
    if args.fake:
        provider_for: ProviderFor = advisor_turn
        model = "fake"
    else:
        try:
            provider = provider_from(settings)
        except RuntimeError as error:
            raise SystemExit(f"{error} Or start it with --fake to answer without a model.") from None
        on_close.append(provider)
        provider_for = lambda _text, _situation: provider  # noqa: E731
        model = settings.model
    service = Service(settings.history_db, provider_for, model)
    log.info("history.db: %s", settings.history_db)
    uvicorn.run(create_app(service, on_close=on_close), host=HOST, port=settings.port, log_level="info")


if __name__ == "__main__":
    main()
