"""FastAPI dependencies for the routers (as in tau-app).

The ``Service`` is made by ``create_app`` and kept on ``app.state``; routers get the Store, Runs and
a looked-up Conversation by type annotation:

    def route(store: StoreDep, conversation: ConversationDep): ...

While history.db can't be used (missing, or at another schema version) they answer 503 with the reason.
"""

from __future__ import annotations

from typing import Annotated

from fastapi import Depends, HTTPException, Request

from backend.agent.runner import Runs
from backend.db import HistoryDbError
from backend.db.store import Conversation, Store
from backend.service import Ready, Service


def get_service(request: Request) -> Service:
    return request.app.state.service


ServiceDep = Annotated[Service, Depends(get_service)]


def get_ready(service: ServiceDep) -> Ready:
    try:
        return service.ready()
    except HistoryDbError as error:
        raise HTTPException(503, str(error)) from None


def get_store(ready: Annotated[Ready, Depends(get_ready)]) -> Store:
    return ready.store


def get_runs(ready: Annotated[Ready, Depends(get_ready)]) -> Runs:
    return ready.runs


StoreDep = Annotated[Store, Depends(get_store)]
RunsDep = Annotated[Runs, Depends(get_runs)]


def get_conversation(conversation_id: str, store: StoreDep) -> Conversation:
    """The Conversation named by the path's ``conversation_id``, or 404."""
    conversation = store.get(conversation_id)
    if not conversation:
        raise HTTPException(404, "Conversation not found.")
    return conversation


ConversationDep = Annotated[Conversation, Depends(get_conversation)]
