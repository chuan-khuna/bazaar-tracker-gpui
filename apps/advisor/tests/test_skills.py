"""The skills: SKILL.md parsing, the catalog in the system prompt, and the load_skill tool."""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

import pytest

from agents.advisor.agent import PROMPT
from agents.advisor.skills import SKILLS, Skill, SkillError, catalog, load_skill_tool, load_skills, parse_skill

COMBAT = "---\nname: combat\ndescription: How a fight runs.\n---\n\n# Combat\n\nItems fire on cooldowns.\n"


def test_parse_skill() -> None:
    assert parse_skill(COMBAT, "combat") == Skill("combat", "How a fight runs.", "# Combat\n\nItems fire on cooldowns.")
    # Windows line endings read the same.
    assert parse_skill(COMBAT.replace("\n", "\r\n"), "combat").name == "combat"


@pytest.mark.parametrize(
    ("text", "error"),
    [
        ("# Combat\n", "no front matter"),
        ("---\nname: combat\n---\nBody\n", "needs 'name' and 'description'"),
        ("---\nname: fights\ndescription: x\n---\nBody\n", "must be the folder's name"),
        ("---\nname: combat\ndescription: x\n---\n\n", "body is empty"),
    ],
)
def test_parse_skill_refuses(text: str, error: str) -> None:
    with pytest.raises(SkillError, match=error):
        parse_skill(text, "combat")


def test_load_skills_and_catalog(tmp_path: Path) -> None:
    for name in ("rules", "combat"):
        (tmp_path / name).mkdir()
        (tmp_path / name / "SKILL.md").write_text(COMBAT.replace("combat", name), encoding="utf-8")
    skills = load_skills(tmp_path)
    assert [s.name for s in skills] == ["combat", "rules"]
    assert catalog(skills) == "- combat: How a fight runs.\n- rules: How a fight runs."
    assert load_skills(tmp_path / "missing") == ()


def test_load_skill_tool() -> None:
    tool = load_skill_tool((parse_skill(COMBAT, "combat"),))
    result = asyncio.run(tool.execute("call-1", {"name": "combat"}))
    assert result.details == {"name": "combat", "content": "# Combat\n\nItems fire on cooldowns."}
    assert json.loads(result.content[0].text)["content"].startswith("# Combat")
    with pytest.raises(ValueError):
        asyncio.run(tool.execute("call-2", {"name": "economy"}))


def test_the_shipped_skills_are_in_the_prompt() -> None:
    assert SKILLS, "skills/ has no SKILL.md"
    for skill in SKILLS:
        assert f"- {skill.name}: {skill.description}" in PROMPT
