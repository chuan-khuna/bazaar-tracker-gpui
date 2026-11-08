"""Turn a plain async function into a tau_agent ``AgentTool``.

tau_agent hands a tool the model's arguments as-is, without checking them against
the schema, so ``define_tool`` checks them first (required keys, types, enums).
The tool's ``run`` gets the checked arguments and returns a JSON-able value,
which goes back to the model as JSON text and to the UI as ``details``.

Raise ``ValueError`` from ``run`` for a failure the model should read and recover
from: the loop turns any exception into an error tool result.

A tool defined with ``terminate=True`` ends the reply once it succeeds (like Flue's
``terminate: true``). tau_agent carries ``AgentToolResult.terminate`` but its loop
never reads it, so the backend runner stops the run itself: see ``ends_reply``.
"""

from __future__ import annotations

import json
from collections.abc import Awaitable, Callable, Mapping
from typing import Any

from tau_agent import AgentTool, AgentToolResult, TextContent, ToolResultMessage

type ToolRun = Callable[[dict[str, Any]], Awaitable[Any]]

_TYPE_CHECKS: dict[str, Callable[[object], bool]] = {
    "string": lambda v: isinstance(v, str),
    "number": lambda v: isinstance(v, (int, float)) and not isinstance(v, bool),
    "integer": lambda v: isinstance(v, int) and not isinstance(v, bool),
    "boolean": lambda v: isinstance(v, bool),
    "array": lambda v: isinstance(v, list),
    "object": lambda v: isinstance(v, dict),
}

# Names of the tools defined with terminate=True.
_TERMINATING: set[str] = set()


def ends_reply(results: list[ToolResultMessage]) -> bool:
    """Whether a turn's tool results include a successful terminating tool: the reply stops there."""
    return any(r.tool_name in _TERMINATING and not r.is_error for r in results)


def define_tool(
    *,
    name: str,
    description: str,
    properties: Mapping[str, dict[str, Any]],
    required: tuple[str, ...] = (),
    run: ToolRun,
    terminate: bool = False,
) -> AgentTool:
    """An ``AgentTool`` whose input is an object with ``properties``. ``terminate``: see the module doc."""
    if terminate:
        _TERMINATING.add(name)

    async def execute(
        tool_call_id: str,
        arguments: Mapping[str, Any],
        signal: object | None = None,
        on_update: object | None = None,
    ) -> AgentToolResult:
        args = _check_arguments(dict(arguments), properties, required)
        output = await run(args)
        return AgentToolResult(
            content=[TextContent(text=json.dumps(output, ensure_ascii=False))],
            details=output,
            terminate=terminate or None,
        )

    return AgentTool(
        name=name,
        label=name,
        description=description,
        parameters={
            "type": "object",
            "properties": dict(properties),
            "required": list(required),
            "additionalProperties": False,
        },
        execute_fn=execute,
    )


def _check_arguments(
    args: dict[str, Any],
    properties: Mapping[str, dict[str, Any]],
    required: tuple[str, ...],
) -> dict[str, Any]:
    # Some models send null for an omitted optional argument.
    args = {k: v for k, v in args.items() if v is not None}
    missing = [key for key in required if key not in args]
    if missing:
        raise ValueError(f"Missing required argument(s): {', '.join(missing)}.")
    unknown = [key for key in args if key not in properties]
    if unknown:
        raise ValueError(f"Unknown argument(s): {', '.join(unknown)}.")
    for key, value in args.items():
        schema = properties[key]
        check = _TYPE_CHECKS.get(schema.get("type", ""))
        if check and not check(value):
            raise ValueError(f"Argument '{key}' must be a {schema['type']}.")
        if "enum" in schema and value not in schema["enum"]:
            raise ValueError(f"Argument '{key}' must be one of: {', '.join(map(str, schema['enum']))}.")
    return args
