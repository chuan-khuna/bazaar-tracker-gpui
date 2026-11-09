"""The Advisor's tools, each built with ``define_tool``.

``find_card``, ``rate_options``, ``synergy``, ``suggest``, ``shop_plan``, ``arrangement`` and
``similar_runs`` read the run's situation and sources, so ``tools_for`` builds them per run;
``follow_ups``, ``ask_user`` and ``load_skill`` (the game knowledge in ``skills/``) need neither. The numbers come from ``knowledge/``: a tool checks its
arguments (``ValueError`` on bad ones), names cards to ids through the card catalog, and returns
JSON with ``n`` and the seasons used.
"""

from __future__ import annotations

from agents.advisor.situation import Situation
from agents.advisor.skills import SKILLS, load_skill_tool
from agents.advisor.sources import Sources
from agents.advisor.tools import arrangement, find_card, rate_options, shop_plan, similar_runs, suggest, synergy
from agents.advisor.tools.context import ToolContext
from agents.advisor.tools.follow_ups import follow_ups
from agents.common.tools import ask_user
from tau_agent import AgentTool

TOOL_NAMES = (
    "find_card",
    "rate_options",
    "synergy",
    "suggest",
    "shop_plan",
    "arrangement",
    "similar_runs",
    "follow_ups",
    "ask_user",
    "load_skill",
)


def tools_for(situation: Situation | None, sources: Sources) -> tuple[AgentTool, ...]:
    """The Advisor's tools for one run."""
    context = ToolContext(situation, sources)
    return (
        find_card.make(context),
        rate_options.make(context),
        synergy.make(context),
        suggest.make(context),
        shop_plan.make(context),
        arrangement.make(context),
        similar_runs.make(context),
        follow_ups,
        ask_user,
        load_skill_tool(SKILLS),
    )


__all__ = ["TOOL_NAMES", "ToolContext", "tools_for"]
