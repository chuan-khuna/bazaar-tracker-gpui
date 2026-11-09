"""What the model gets of a Conversation: each question with its situation, and old results marked old.

The situation changes between questions (a new Offer, a bought card, the next hour), but the
Conversation keeps every earlier question, answer and tool result. Sent as stored, the old results
(an earlier Offer's ratings, an earlier day's shop odds) sit closer to the new question than the
situation does, and the model answers from them. So:

- the question being asked goes with its situation, rendered just above it (``question_message``);
- an earlier question asked with another situation is marked with when it was asked;
- the result of a tool that read another situation (``SITUATION_TOOLS``) is replaced by a line
  saying it was left out, so the model calls the tool again. Its tool call stays, so every call
  still has its result. Results under the same situation as now are kept, so a follow-up about
  the same Offer doesn't run the tools again.

The stored messages are never changed: this is only what the model is sent.
"""

from __future__ import annotations

import json
from collections.abc import Sequence

from tau_agent import AgentMessage, TextContent, ToolResultMessage, UserMessage

from agents.advisor.situation import Situation, SituationError, parse_situation, render_situation

NO_SITUATION = "## Situation for this question\n\nThe app sent no situation with this message."

# The tools whose results depend on the situation (its Offer, Board, day or skills). find_card,
# follow_ups and ask_user don't.
SITUATION_TOOLS = frozenset({"rate_options", "synergy", "suggest", "shop_plan", "arrangement", "similar_runs"})


def situation_section(situation: object | None) -> str:
    """The situation as the section above a question: rendered, or why there isn't one."""
    if situation is None:
        return NO_SITUATION
    try:
        return render_situation(parse_situation(situation), "Situation for this question")
    except SituationError as error:
        return f"## Situation for this question\n\n{error} Ask the player what you need to know."


def question_message(text: str, situation: object | None) -> UserMessage:
    """The question as the model gets it: the situation sent with it, then the question."""
    return UserMessage(content=f"{situation_section(situation)}\n\n## Question\n\n{text}")


def model_history(stored: Sequence[tuple[AgentMessage, object | None]], situation: object | None) -> list[AgentMessage]:
    """The stored messages, each with its question's situation, as the model gets them before a
    question asked with ``situation``."""
    now = _key(situation)
    out: list[AgentMessage] = []
    # The situation of the question the messages so far answer; None before the first question.
    asked_with: object | None = None
    for message, sent in stored:
        if isinstance(message, UserMessage):
            asked_with = sent
            if _key(sent) != now:
                message = message.model_copy(update={"content": f"[Asked {_when(sent)}.]\n\n{message.text}"})
        elif (
            isinstance(message, ToolResultMessage)
            and message.tool_name in SITUATION_TOOLS
            and _key(asked_with) != now
        ):
            note = (
                f"[This {message.tool_name} result was for an earlier situation ({_when(asked_with)}) "
                f"and is left out. Call {message.tool_name} again for the situation now.]"
            )
            message = message.model_copy(update={"content": [TextContent(text=note)], "details": None})
        out.append(message)
    return out


def _key(situation: object | None) -> str:
    return json.dumps(situation, sort_keys=True, ensure_ascii=False)


def _when(situation: object | None) -> str:
    """When a question was asked, from its situation, for the marks on old messages."""
    if situation is None:
        return "with no situation"
    try:
        parsed: Situation = parse_situation(situation)
    except SituationError:
        return "with a situation that wasn't valid"
    parts = []
    if parsed.event is not None:
        parts.append(f"about Event #{parsed.event.number}" if parsed.event.number is not None else "about a past Event")
    if parsed.day is not None:
        parts.append(f"on Day {parsed.day}" + (f", Hour {parsed.hour}" if parsed.hour is not None else ""))
    if parsed.game_state:
        parts.append(f"in {parsed.game_state}")
    where = " ".join(parts) if parts else "in another situation"
    return f"{where}, with a different situation from now"
