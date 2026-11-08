"""The Advisor's ``AgentSpec``. ``prepare`` checks the situation the app sent, renders it into the
system prompt, and builds the tools for the run around it."""

from __future__ import annotations

from pathlib import Path

from agents.advisor.situation import SituationError, parse_situation, render_situation
from agents.advisor.sources import Sources, default_sources
from agents.advisor.tools import tools_for
from agents.core.registry import SITUATION_KEY, AgentSpec, RunContext, RunSetup

PROMPT = (Path(__file__).parent / "prompt.md").read_text(encoding="utf-8").strip()

NO_SITUATION = "## Situation\n\nThe app sent no situation with this message."

# State key holding the run's ``Sources`` (history.db and the card catalog): a live object, not
# JSON, and never saved. Without it the tools use ``default_sources()``, from the config.
SOURCES_KEY = "advisor_sources"


def system_prompt(situation: object | None) -> str:
    """The prompt, with the situation (the app's JSON) rendered as its ``## Situation`` section."""
    if situation is None:
        return f"{PROMPT}\n\n{NO_SITUATION}"
    try:
        return f"{PROMPT}\n\n{render_situation(parse_situation(situation))}"
    except SituationError as error:
        return f"{PROMPT}\n\n## Situation\n\n{error} Ask the player what you need to know."


def _prepare(context: RunContext) -> RunSetup:
    raw = context.state.get(SITUATION_KEY)
    try:
        situation = parse_situation(raw) if raw is not None else None
    except SituationError:
        situation = None
    sources = context.state.get(SOURCES_KEY)
    if not isinstance(sources, Sources):
        sources = default_sources()
    return RunSetup(system_prompt(raw), tools_for(situation, sources))


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
)
