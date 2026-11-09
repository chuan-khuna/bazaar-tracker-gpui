"""The Advisor's skills: how to play The Bazaar, in Markdown the model loads when it needs it.

Each skill is ``skills/<name>/SKILL.md``: a front matter block with ``name`` (the folder's name) and
``description`` (one line saying when to load it), then the body. The system prompt lists every
skill's name and description (``catalog``); the model calls ``load_skill`` for a body, so the prompt
stays short and the game knowledge is read only when a question needs it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from agents.core.tooling import define_tool

SKILLS_DIR = Path(__file__).parent / "skills"

_FRONT_MATTER = re.compile(r"\A---\n(.*?)\n---\n(.*)\Z", re.DOTALL)
_NAME = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")


class SkillError(ValueError):
    """A SKILL.md that can't be read: no front matter, a missing field or a name that isn't its folder's."""


@dataclass(frozen=True, slots=True)
class Skill:
    name: str
    description: str
    body: str


def parse_skill(text: str, folder: str) -> Skill:
    """A skill from its SKILL.md text; ``folder`` is the name of the folder it is in."""
    match = _FRONT_MATTER.match(text.replace("\r\n", "\n"))
    if match is None:
        raise SkillError(f"{folder}/SKILL.md: no front matter between '---' lines.")
    fields: dict[str, str] = {}
    for line in match.group(1).splitlines():
        key, sep, value = line.partition(":")
        if sep:
            fields[key.strip()] = value.strip()
    name, description = fields.get("name", ""), fields.get("description", "")
    if not name or not description:
        raise SkillError(f"{folder}/SKILL.md: front matter needs 'name' and 'description'.")
    if name != folder or not _NAME.match(name):
        raise SkillError(f"{folder}/SKILL.md: name '{name}' must be the folder's name, in kebab-case.")
    body = match.group(2).strip()
    if not body:
        raise SkillError(f"{folder}/SKILL.md: the body is empty.")
    return Skill(name, description, body)


def load_skills(root: Path = SKILLS_DIR) -> tuple[Skill, ...]:
    """Every skill under ``root``, by name."""
    if not root.is_dir():
        return ()
    return tuple(
        parse_skill(path.read_text(encoding="utf-8"), path.parent.name)
        for path in sorted(root.glob("*/SKILL.md"))
    )


def catalog(skills: tuple[Skill, ...]) -> str:
    """The skills as the system prompt lists them: one ``- name: description`` line each."""
    return "\n".join(f"- {skill.name}: {skill.description}" for skill in skills)


def load_skill_tool(skills: tuple[Skill, ...]):
    """The ``load_skill`` tool over ``skills``."""
    by_name = {skill.name: skill for skill in skills}

    async def _load(args: dict[str, Any]) -> dict[str, Any]:
        skill = by_name.get(args["name"])
        if skill is None:
            raise ValueError(f"No skill named '{args['name']}'. Skills: {', '.join(by_name)}.")
        return {"name": skill.name, "content": skill.body}

    return define_tool(
        name="load_skill",
        description=(
            "Load one of the skills listed in the system prompt: how The Bazaar plays (rules, combat, "
            "strategy, a hero's builds). Load what the question needs before answering; don't load a skill "
            "you already loaded in this Conversation."
        ),
        properties={"name": {"type": "string", "enum": list(by_name)}},
        required=("name",),
        run=_load,
    )


SKILLS = load_skills()
