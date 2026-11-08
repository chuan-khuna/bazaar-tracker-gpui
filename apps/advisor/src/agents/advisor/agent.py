"""The Advisor's ``AgentSpec``. ``prepare`` puts the situation the app sent into the system prompt."""

from __future__ import annotations

import json
from pathlib import Path

from agents.advisor.tools import TOOLS
from agents.core.registry import SITUATION_KEY, AgentSpec, RunContext, RunSetup

PROMPT = (Path(__file__).parent / "prompt.md").read_text(encoding="utf-8").strip()

NO_SITUATION = "## Situation\n\nThe app sent no situation with this message."


def system_prompt(situation: object | None) -> str:
    """The prompt, with ``situation`` (the app's JSON) appended as a fenced block."""
    if situation is None:
        return f"{PROMPT}\n\n{NO_SITUATION}"
    text = json.dumps(situation, ensure_ascii=False, indent=2)
    return f"{PROMPT}\n\n## Situation\n\n```json\n{text}\n```"


def _prepare(context: RunContext) -> RunSetup:
    return RunSetup(system_prompt(context.state.get(SITUATION_KEY)), TOOLS)


ADVISOR = AgentSpec(
    id="advisor",
    name="Advisor",
    description="Helps decide during a Run of The Bazaar, from Scraped runs and the card catalog.",
    examples=(
        "Which of these should I take?",
        "What goes with my Board?",
        "Which shop should I visit?",
    ),
    system=PROMPT,
    tools=TOOLS,
    prepare=_prepare,
)
