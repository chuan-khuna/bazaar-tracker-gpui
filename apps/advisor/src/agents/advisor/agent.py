"""The Advisor's ``AgentSpec``. ``prepare`` checks the situation the app sent and builds the tools
for the run around it. The situation goes to the model beside the question, not in the system prompt
(``history.py``), so the prompt stays the same from question to question."""

from __future__ import annotations

from pathlib import Path

from agents.advisor.history import model_history, question_message
from agents.advisor.situation import SituationError, parse_situation
from agents.advisor.skills import SKILLS, catalog
from agents.advisor.sources import Sources, default_sources
from agents.advisor.tools import tools_for
from agents.core.registry import SITUATION_KEY, AgentSpec, RunContext, RunSetup

# The rules in prompt.md, then the skills the model can load (``skills.py``).
PROMPT = (
    (Path(__file__).parent / "prompt.md").read_text(encoding="utf-8").strip()
    + "\n\n## Skills\n\nLoad one with load_skill when the question needs it:\n"
    + catalog(SKILLS)
)

# State key holding the run's ``Sources`` (history.db and the card catalog): a live object, not
# JSON, and never saved. Without it the tools use ``default_sources()``, from the config.
SOURCES_KEY = "advisor_sources"


def _prepare(context: RunContext) -> RunSetup:
    raw = context.state.get(SITUATION_KEY)
    try:
        situation = parse_situation(raw) if raw is not None else None
    except SituationError:
        situation = None
    sources = context.state.get(SOURCES_KEY)
    if not isinstance(sources, Sources):
        sources = default_sources()
    return RunSetup(PROMPT, tools_for(situation, sources))


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
    tools=tools_for(None, Sources()),
    prepare=_prepare,
    question=question_message,
    history=model_history,
)
