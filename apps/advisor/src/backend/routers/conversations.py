"""A Run's Conversations: list, start, delete."""

from __future__ import annotations

from typing import Any

from fastapi import APIRouter, HTTPException

from backend.deps import ConversationDep, RunsDep, StoreDep
from backend.schemas import StartConversation, checked_situation, question

router = APIRouter(tags=["conversations"])


def _check_run(store: StoreDep, run_id: str) -> None:
    if not store.run_exists(run_id):
        raise HTTPException(404, "Run not found.")


@router.get("/runs/{run_id}/conversations")
def list_conversations(run_id: str, store: StoreDep) -> list[dict[str, Any]]:
    """The Run's Conversations, newest first."""
    _check_run(store, run_id)
    return [c.to_json() for c in store.conversations(run_id)]


@router.post("/runs/{run_id}/conversations", status_code=201)
def start_conversation(run_id: str, body: StartConversation, store: StoreDep) -> dict[str, Any]:
    """Records a Conversation titled by its first question, at the situation's Day and Hour. The app
    then sends that question to its messages."""
    _check_run(store, run_id)
    text = question(body.text)
    situation = checked_situation(body.situation)
    day = situation.day if situation else None
    hour = situation.hour if situation else None
    return store.create(run_id, text, day, hour).to_json()


@router.delete("/conversations/{conversation_id}", status_code=204)
def delete_conversation(conversation: ConversationDep, store: StoreDep, runs: RunsDep) -> None:
    if runs.is_running(conversation.id):
        raise HTTPException(409, "Stop the running reply first.")
    store.delete(conversation.id)
