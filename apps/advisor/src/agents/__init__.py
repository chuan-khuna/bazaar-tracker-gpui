"""Agents the Advisor serves, built on tau_agent and tau_ai (the way learning-agent's tau-app builds them).

``AGENTS`` is the registry: an agent is ``agents/<id>/`` with its ``AgentSpec`` listed here.
Never name an agent ``common`` or ``core``.
"""

from __future__ import annotations

from agents.advisor import ADVISOR
from agents.core.registry import AgentSpec, build_harness

AGENTS: dict[str, AgentSpec] = {spec.id: spec for spec in (ADVISOR,)}

__all__ = ["AGENTS", "AgentSpec", "build_harness"]
