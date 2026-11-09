"""Ask the user one multiple-choice question, then end the reply (copied from tau-app).

Nothing waits: the tool is defined with ``terminate=True``, so the backend stops the
run after it, the conversation goes idle, and the answer arrives later as the user's
next message: a choice's label, or their own words. With ``multiple`` the user may
pick several, and the answer lists them one per line as ``- <label>`` (free text, if
any, as a last line). The tracker's Advisor tab renders this call as a card from its
arguments, so the argument names are tau-app's.
"""

from __future__ import annotations

from typing import Any

from agents.core.tooling import define_tool

_MIN_OPTIONS = 2
_MAX_OPTIONS = 6


async def _ask_user(args: dict[str, Any]) -> dict[str, Any]:
    if not args["question"].strip():
        raise ValueError("Argument 'question' must not be empty.")
    options = args["options"]
    if not _MIN_OPTIONS <= len(options) <= _MAX_OPTIONS:
        raise ValueError(f"Argument 'options' must have {_MIN_OPTIONS}-{_MAX_OPTIONS} choices.")
    for option in options:
        if not (
            isinstance(option, dict)
            and isinstance(option.get("id"), str)
            and option["id"]
            and isinstance(option.get("label"), str)
            and option["label"]
        ):
            raise ValueError("Each option must be an object with a non-empty 'id' and 'label'.")
    return {"status": "waiting_for_user"}


ask_user = define_tool(
    name="ask_user",
    description=(
        "Ask the user one question with 2-6 short choices, then stop and wait. "
        "Use it when you need the user to pick before you can help. "
        "Set multiple to true when several choices can apply at once (e.g. toppings, dietary needs); "
        "leave it false when only one makes sense. "
        "Their answer arrives as their next message: a choice's label, or with multiple, "
        "one '- <label>' line per choice; their own words may appear when allowFreeText is true. "
        "Don't repeat the question or the choices in text."
    ),
    properties={
        "question": {"type": "string"},
        "options": {
            "type": "array",
            "minItems": _MIN_OPTIONS,
            "maxItems": _MAX_OPTIONS,
            "items": {
                "type": "object",
                "properties": {"id": {"type": "string"}, "label": {"type": "string"}},
                "required": ["id", "label"],
                "additionalProperties": False,
            },
        },
        "multiple": {"type": "boolean", "default": False},
        "allowFreeText": {"type": "boolean", "default": True},
    },
    required=("question", "options"),
    run=_ask_user,
    terminate=True,
)
