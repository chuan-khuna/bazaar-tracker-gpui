"""suggest: the model's ranking, drawn as the recommendation card. The model gives only the order and a
reason for each card; the tool adds each card's numbers (as rate_options), its source runs (Board,
skills, result and link, for the card's hover and expand) and the source line. It doesn't end the
reply. The model reads the ranking and numbers; the source runs go only to the app."""

from __future__ import annotations

from typing import Any

import knowledge
from agents.advisor.tools.context import ToolContext, hero_of, run_json, sample_json, text_arg
from agents.advisor.tools.rate_options import option_json, rate
from agents.core.tooling import define_tool

MAX_RANKED = 10
THIN_MARK = "⚠"


def source_line(sample: knowledge.Sample, profiles: int) -> str:
    """``15 Jules runs · S20 · 1 profile``, with ``· thin ⚠`` under the sample's ``min_runs``."""
    if not sample.n:
        return f"no Scraped {sample.hero} runs · thin {THIN_MARK}"
    seasons = f"S{sample.seasons[0].season}" if len(sample.seasons) == 1 else sample.label
    line = f"{sample.n} {sample.hero} run{'' if sample.n == 1 else 's'} · {seasons} · "
    line += f"{profiles} profile{'' if profiles == 1 else 's'}"
    return line + (f" · thin {THIN_MARK}" if sample.thin else "")


def _ranked(args: dict[str, Any]) -> list[tuple[str, str]]:
    ranked = args["ranked"]
    if not 1 <= len(ranked) <= MAX_RANKED:
        raise ValueError(f"Argument 'ranked' must have 1-{MAX_RANKED} cards.")
    found: list[tuple[str, str]] = []
    for entry in ranked:
        card = entry.get("card") if isinstance(entry, dict) else None
        reason = entry.get("reason") if isinstance(entry, dict) else None
        if not (isinstance(card, str) and card.strip() and isinstance(reason, str) and reason.strip()):
            raise ValueError("Each entry of 'ranked' must be an object with a non-empty 'card' and 'reason'.")
        found.append((card.strip(), reason.strip()))
    if len({card.lower() for card, _ in found}) < len(found):
        raise ValueError("A card appears twice in 'ranked'.")
    return found


async def _suggest(context: ToolContext, args: dict[str, Any]) -> dict[str, Any]:
    title = text_arg(args, "title")
    ranked = _ranked(args)
    hero = hero_of(context, args)
    cards, board, ratings = rate(context, [name for name, _ in ranked], hero)
    sample = ratings.sample
    rows = []
    run_ids: list[str] = []
    for rank, ((_, reason), card, rating) in enumerate(zip(ranked, cards, ratings.options), start=1):
        row = {"rank": rank, **option_json(context, card, rating, sample.n, board), "reason": reason}
        rows.append(row)
        run_ids += rating.run_ids + rating.offered_run_ids
        run_ids += [run_id for fit in rating.with_board for run_id in fit.run_ids]
    with context.sources.connection() as connection:
        runs = knowledge.run_summaries(connection, run_ids)
        profiles = knowledge.profile_count(connection, sample.run_ids)
    return {
        "title": title,
        "hero": hero,
        **sample_json(sample),
        "source": source_line(sample, profiles),
        "profiles": profiles,
        "ranked": rows,
        "runs": {run_id: run_json(context, run) for run_id, run in runs.items()},
    }


def for_the_model(result: dict[str, Any]) -> dict[str, Any]:
    """The result less the source runs and run ids, which only the app draws."""

    def strip(value: Any) -> Any:
        if isinstance(value, dict):
            return {k: strip(v) for k, v in value.items() if not k.endswith("run_ids") and k != "runs"}
        if isinstance(value, list):
            return [strip(v) for v in value]
        return value

    return strip(result) | {"shown": "The recommendation card is drawn under your reply; don't repeat it as a list."}


def make(context: ToolContext):
    async def run(args: dict[str, Any]) -> dict[str, Any]:
        return await _suggest(context, args)

    return define_tool(
        name="suggest",
        description=(
            "Draw the recommendation card: your ranking of cards (best first), each with a one-line reason. "
            "Call it whenever you rank an Offer or skills. The card shows each option's numbers and their "
            "source runs, which this tool adds; you give only the order and the reasons. It does not end "
            "your reply: still write the answer, then call follow_ups."
        ),
        properties={
            "title": {"type": "string", "description": "e.g. 'Shop · Day 5'"},
            "ranked": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_RANKED,
                "items": {
                    "type": "object",
                    "properties": {"card": {"type": "string"}, "reason": {"type": "string"}},
                    "required": ["card", "reason"],
                    "additionalProperties": False,
                },
            },
            "hero": {"type": "string", "description": "default: the situation's hero"},
        },
        required=("title", "ranked"),
        run=run,
        model_view=for_the_model,
    )
