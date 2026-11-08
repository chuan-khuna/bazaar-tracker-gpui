"""The Advisor's tools on the record's example: 15 Jules runs, a Board of Giant Lollipop and Pizza
Cutter, and an Offer of Decanter, Laurel's Bouncy Ball and Cheese Wheel (``sources`` is in conftest.py,
``CATALOG`` and ``SITUATION`` in scraped_runs.py)."""

from __future__ import annotations

import asyncio
import json
from pathlib import Path
from typing import Any

import pytest
from scraped_runs import BALL, CATALOG, CUTTER, LOLLIPOP, SITUATION, record_run_ids

from agents.advisor.situation import parse_situation
from agents.advisor.sources import Sources
from agents.advisor.tools import TOOL_NAMES, tools_for
from agents.core.tooling import ends_reply
from knowledge.cards import CardCatalog


def tools(sources: Sources, situation: object | None = SITUATION) -> dict[str, Any]:
    parsed = parse_situation(situation) if situation is not None else None
    return {tool.name: tool for tool in tools_for(parsed, sources)}


def call(tool: Any, arguments: dict[str, Any]) -> tuple[dict[str, Any], Any]:
    """The tool's whole result (``details``) and what the model reads."""
    result = asyncio.run(tool.execute("call-1", arguments))
    return result.details, json.loads(result.content[0].text)


def test_the_tools() -> None:
    assert TOOL_NAMES == ("find_card", "rate_options", "synergy", "suggest", "follow_ups", "ask_user")
    assert tuple(tool.name for tool in tools_for(None, Sources())) == TOOL_NAMES


# --- rate_options ---------------------------------------------------------------------------------


def test_rate_options_defaults_to_the_offer(sources: Sources) -> None:
    result, _ = call(tools(sources)["rate_options"], {})

    assert (result["hero"], result["n"], result["seasons"], result["thin"]) == ("Jules", 15, [{"season": 20, "runs": 15}], False)
    options = {option["card"]: option for option in result["options"]}
    assert list(options) == ["Decanter", "Laurel's Bouncy Ball", "Cheese Wheel"]

    ball = options["Laurel's Bouncy Ball"]
    assert (ball["id"], ball["kind"], ball["size"], ball["tier"], ball["fits"]) == (BALL, "merchant_item", "Small", "Gold", True)
    assert ball["runs_with"] == {"n": 4, "of": 15, "run_ids": ["jules-01", "jules-02", "jules-03", "jules-04"]}
    assert (ball["picks"]["offered"], ball["picks"]["taken"], ball["picks"]["take_rate"]) == (15, 7, 0.47)
    assert ball["picks"]["taken_run_ids"] == record_run_ids()[:7]
    with_board = {fit["card"]: fit for fit in ball["with_board"]}
    assert (with_board["Giant Lollipop"]["n"], with_board["Giant Lollipop"]["of"], with_board["Giant Lollipop"]["lift"]) == (3, 5, 2.25)
    assert (with_board["Pizza Cutter"]["n"], with_board["Pizza Cutter"]["of"], with_board["Pizza Cutter"]["lift"]) == (3, 4, 2.81)
    assert with_board["Pizza Cutter"]["run_ids"] == ["jules-01", "jules-02", "jules-04"]

    assert [fit["n"] for fit in options["Decanter"]["with_board"]] == [2, 3]
    assert [fit["n"] for fit in options["Cheese Wheel"]["with_board"]] == [1, 2]
    assert (options["Decanter"]["picks"]["offered"], options["Cheese Wheel"]["picks"]["taken"]) == (16, 11)
    assert not any(option["no_data"] for option in options.values())


def test_rate_options_by_name_with_a_day_and_fit(sources: Sources) -> None:
    situation = SITUATION | {"board": SITUATION["board"] + [{"name": "Decanter", "size": "Large", "slot": 5}]}
    result, _ = call(tools(sources, situation)["rate_options"], {"options": ["decanter", "Cheese Wheel"], "day": 2})

    decanter, cheese = result["options"]
    # 10 slots: 3 + 1 + 3 used, so the Medium Decanter fits and a Small Cheese Wheel does too.
    assert (decanter["fits"], cheese["fits"]) == (True, True)
    # Only day 2's offers: the 16th Decanter offer and none of Cheese Wheel's 15.
    assert (decanter["picks"]["offered"], cheese["picks"]["offered"]) == (1, 0)
    # The Board's Decanter (resolved by name) isn't compared with itself.
    assert [fit["id"] for fit in decanter["with_board"]] == [LOLLIPOP, CUTTER]


def test_rate_options_full_board_and_no_data(sources: Sources) -> None:
    full = [{"id": LOLLIPOP, "name": "Giant Lollipop", "size": "Large", "slot": 0}] * 3 + [
        {"id": CUTTER, "name": "Pizza Cutter", "size": "Small", "slot": 9}
    ]
    result, _ = call(tools(sources, SITUATION | {"board": full})["rate_options"], {"options": ["Decanter", "Cutlass"]})
    decanter, cutlass = result["options"]
    assert decanter["fits"] is False
    assert (cutlass["no_data"], cutlass["runs_with"]["n"], cutlass["picks"]["offered"]) == (True, 0, 0)


def test_rate_options_refuses_bad_arguments(sources: Sources) -> None:
    rate = tools(sources)["rate_options"]
    for arguments, message in [
        ({"options": []}, "1-10"),
        ({"options": ["  "]}, "list of card names"),
        ({"options": ["No Such Card"]}, "No card named 'No Such Card'"),
        ({"day": 0}, "1-30"),
        ({"options": "Decanter"}, "must be a array"),
        ({"bogus": 1}, "Unknown argument"),
    ]:
        with pytest.raises(ValueError, match=message):
            call(rate, arguments)
    with pytest.raises(ValueError, match="no Offer"):
        call(tools(sources, {"hero": "Jules"})["rate_options"], {})
    with pytest.raises(ValueError, match="No hero"):
        call(tools(sources, None)["rate_options"], {"options": ["Decanter"]})


# --- synergy ----------------------------------------------------------------------------------------


def test_synergy(sources: Sources) -> None:
    result, _ = call(tools(sources)["synergy"], {"cards": ["Giant Lollipop", "Pizza Cutter"]})

    assert (result["n"], result["seasons"], result["min_support"]) == (15, [{"season": 20, "runs": 15}], 3)
    lollipop, cutter = result["cards"]
    assert (lollipop["card"], lollipop["n"]) == ("Giant Lollipop", 5)
    assert [(p["card"], p["n"], p["lift"]) for p in lollipop["partners"]] == [("Laurel's Bouncy Ball", 3, 2.25)]
    partners = {p["card"]: p for p in cutter["partners"]}
    assert set(partners) == {"Laurel's Bouncy Ball", "Decanter"}
    assert (partners["Laurel's Bouncy Ball"]["lift"], partners["Decanter"]["lift"]) == (2.81, 2.25)
    assert partners["Decanter"]["shared_mechanics"] == ["Damage", "DamageAmount"]
    assert partners["Decanter"]["run_ids"] == ["jules-01", "jules-04", "jules-07"]
    assert partners["Decanter"]["on_your_board"] is False


def test_synergy_refuses_bad_arguments(sources: Sources) -> None:
    synergy = tools(sources)["synergy"]
    for arguments, message in [
        ({}, "Missing required"),
        ({"cards": []}, "1-5"),
        ({"cards": ["Decanter"], "limit": 0}, "1-20"),
        ({"cards": ["Nope"]}, "No card named"),
    ]:
        with pytest.raises(ValueError, match=message):
            call(synergy, arguments)


# --- find_card ----------------------------------------------------------------------------------------


def test_find_card(sources: Sources) -> None:
    result, _ = call(tools(sources)["find_card"], {"query": "bouncy"})

    assert (result["n"], result["seasons"], result["hero"]) == (15, [{"season": 20, "runs": 15}], "Jules")
    (ball,) = result["matches"]
    assert (ball["card"], ball["id"], ball["size"], ball["tier"], ball["type"]) == ("Laurel's Bouncy Ball", BALL, "Small", "Silver", "Item")
    assert ball["text"] == "Laurel's Bouncy Ball does a thing"
    assert ball["tags"] == ["Toy", "Haste"]
    assert ball["attributes"] == {"CooldownMax": 5000, "HasteAmount": 1000}
    assert ball["runs_with"] == {"n": 4, "of": 15}


def test_find_card_filters_and_misses(sources: Sources) -> None:
    find = tools(sources, None)["find_card"]
    assert [m["card"] for m in call(find, {"query": "cutlass"})[0]["matches"]] == ["Cutlass"]
    assert call(find, {"query": "cutlass", "hero": "Jules"})[0]["matches"] == []
    assert call(find, {"query": "e", "type": "Skill"})[0]["matches"] == []
    # No hero anywhere: cards, but no run counts.
    found, _ = call(find, {"query": "Decanter"})
    assert (found["n"], found["seasons"]) == (0, []) and "runs_with" not in found["matches"][0]
    with pytest.raises(ValueError, match="must not be empty"):
        call(find, {"query": " "})
    with pytest.raises(ValueError, match="catalog is empty"):
        call(tools(Sources(engine=None, catalog=CardCatalog()), None)["find_card"], {"query": "x"})


def test_find_card_without_history_db(tmp_path: Path) -> None:
    find = tools(Sources(tmp_path / "missing.db", catalog=CATALOG))["find_card"]
    found, _ = call(find, {"query": "Decanter"})
    assert found["n"] == 0 and "No history.db" in found["note"]
    with pytest.raises(ValueError, match="No history.db"):
        call(tools(Sources(tmp_path / "missing.db", catalog=CATALOG))["rate_options"], {})


# --- suggest --------------------------------------------------------------------------------------------


def test_suggest_adds_numbers_and_source_runs(sources: Sources) -> None:
    ranked = [
        {"card": "Laurel's Bouncy Ball", "reason": "Shows up with both your items"},
        {"card": "Decanter", "reason": "Close behind with Pizza Cutter"},
        {"card": "Cheese Wheel", "reason": "Taken a lot, rarely with your items"},
    ]
    suggest = tools(sources)["suggest"]
    result, for_model = call(suggest, {"title": "Shop · Day 5", "ranked": ranked})

    assert (result["title"], result["n"], result["seasons"], result["thin"]) == ("Shop · Day 5", 15, [{"season": 20, "runs": 15}], False)
    assert result["source"] == "15 Jules runs · S20 · 1 profile"
    rows = result["ranked"]
    assert [(r["rank"], r["card"], r["reason"]) for r in rows] == [
        (1, "Laurel's Bouncy Ball", "Shows up with both your items"),
        (2, "Decanter", "Close behind with Pizza Cutter"),
        (3, "Cheese Wheel", "Taken a lot, rarely with your items"),
    ]
    assert rows[0]["runs_with"]["n"] == 4 and rows[0]["picks"]["taken"] == 7
    assert [fit["lift"] for fit in rows[0]["with_board"]] == [2.25, 2.81]

    # Every run behind a number is there to hover and expand: Board, skills, result and link.
    behind = set(rows[0]["runs_with"]["run_ids"]) | set(rows[0]["picks"]["offered_run_ids"])
    assert behind <= set(result["runs"])
    run = result["runs"]["jules-01"]
    assert run["url"] == "https://bazaardb.gg/run/tracker/jules-01"
    assert (run["wins"], run["losses"], run["victory_tier"], run["season"]) == (10, 2, "Gold Victory", 20)
    assert [c["name"] for c in run["board"]] == ["Giant Lollipop", "Pizza Cutter", "Laurel's Bouncy Ball", "Decanter", "TPL-FILLER-0"]
    assert (run["board"][1]["slot"], run["board"][1]["size"]) == (3, 1)

    # The model reads the ranking and numbers, not the source runs.
    assert "runs" not in for_model and "run_ids" not in for_model["ranked"][0]["runs_with"]
    assert for_model["ranked"][0]["runs_with"] == {"n": 4, "of": 15}
    assert for_model["source"] == result["source"]


def test_suggest_and_follow_ups_do_not_end_the_reply(sources: Sources) -> None:
    from tau_agent import ToolResultMessage

    assert not ends_reply([ToolResultMessage(tool_call_id="1", tool_name="suggest", content=[], is_error=False)])
    assert not ends_reply([ToolResultMessage(tool_call_id="1", tool_name="follow_ups", content=[], is_error=False)])
    suggest = asyncio.run(
        tools(sources)["suggest"].execute("c", {"title": "T", "ranked": [{"card": "Decanter", "reason": "r"}]})
    )
    assert not suggest.terminate


def test_suggest_thin_and_no_data(sources: Sources, tmp_path: Path) -> None:
    result, _ = call(
        tools(sources, SITUATION | {"hero": "Vanessa"})["suggest"],
        {"title": "T", "ranked": [{"card": "Cutlass", "reason": "from card text"}]},
    )
    assert (result["n"], result["thin"], result["source"]) == (0, True, "no Scraped Vanessa runs · thin ⚠")
    assert result["ranked"][0]["no_data"] is True and result["runs"] == {}


def test_suggest_refuses_bad_arguments(sources: Sources) -> None:
    suggest = tools(sources)["suggest"]
    for arguments, message in [
        ({"ranked": [{"card": "Decanter", "reason": "r"}]}, "Missing required"),
        ({"title": " ", "ranked": [{"card": "Decanter", "reason": "r"}]}, "must not be empty"),
        ({"title": "T", "ranked": []}, "1-10"),
        ({"title": "T", "ranked": [{"card": "Decanter"}]}, "non-empty 'card' and 'reason'"),
        ({"title": "T", "ranked": [{"card": "Decanter", "reason": "a"}, {"card": "decanter", "reason": "b"}]}, "twice"),
        ({"title": "T", "ranked": [{"card": "Nope", "reason": "r"}]}, "No card named"),
    ]:
        with pytest.raises(ValueError, match=message):
            call(suggest, arguments)


# --- follow_ups -------------------------------------------------------------------------------------------


def test_follow_ups(sources: Sources) -> None:
    follow = tools(sources)["follow_ups"]
    result, _ = call(follow, {"questions": [" Where should I put it? ", "Which skill fits?"]})
    assert result == {"questions": ["Where should I put it?", "Which skill fits?"]}
    for questions, message in [
        (["One?"], "2-3"),
        (["A?", "B?", "C?", "D?"], "2-3"),
        (["Same?", "Same?"], "2-3"),
        (["A?", ""], "non-empty"),
        (["A?", "x" * 121], "at most 120"),
    ]:
        with pytest.raises(ValueError, match=message):
            call(follow, {"questions": questions})
