"""follow_ups: 2-3 questions the player might ask next, shown as buttons under the reply. Once the
reply has text it ends the reply (``ends_answered_reply``): no model call follows, so a model can't
answer again after it. Called before the answer, the reply goes on. It returns no numbers, so it has
no ``n``."""

from __future__ import annotations

from typing import Any

from agents.core.tooling import define_tool

MIN_QUESTIONS = 2
MAX_QUESTIONS = 3
MAX_LENGTH = 120


async def _follow_ups(args: dict[str, Any]) -> dict[str, Any]:
    questions = args["questions"]
    if not all(isinstance(q, str) and q.strip() for q in questions):
        raise ValueError("Argument 'questions' must be a list of non-empty strings.")
    questions = list(dict.fromkeys(q.strip() for q in questions))
    if not MIN_QUESTIONS <= len(questions) <= MAX_QUESTIONS:
        raise ValueError(f"Argument 'questions' must have {MIN_QUESTIONS}-{MAX_QUESTIONS} different questions.")
    if any(len(q) > MAX_LENGTH for q in questions):
        raise ValueError(f"Each question must be at most {MAX_LENGTH} characters.")
    return {"questions": questions}


follow_ups = define_tool(
    name="follow_ups",
    description=(
        "Offer 2-3 short questions the player might ask next, in their words (e.g. 'Where should I put it?'). "
        "They are shown as buttons under your reply. Call it once, last, after your answer: it ends the reply."
    ),
    properties={
        "questions": {
            "type": "array",
            "minItems": MIN_QUESTIONS,
            "maxItems": MAX_QUESTIONS,
            "items": {"type": "string"},
        }
    },
    required=("questions",),
    run=_follow_ups,
    ends_answered_reply=True,
)
