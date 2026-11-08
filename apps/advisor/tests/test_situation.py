"""The situation: checking the app's JSON and rendering it into the system prompt."""

from __future__ import annotations

import pytest
from scraped_runs import SITUATION

from agents.advisor.agent import PROMPT, system_prompt
from agents.advisor.situation import OFFER_KINDS, SituationError, parse_situation, render_situation


def test_a_full_situation_parses() -> None:
    situation = parse_situation(SITUATION | {"opponent": {"name": "Rival", "hero": "Pygmalien", "health": 800, "board": [{"name": "Pigomorph", "slot": 0}], "skills": []}})
    assert situation.hero == "Jules" and situation.day == 5
    assert [card.name for card in situation.board] == ["Giant Lollipop", "Pizza Cutter"]
    assert [card.kind for card in situation.offer] == ["merchant_item"] * 3
    assert situation.free_slots() == 6
    assert situation.opponent.board[0].name == "Pigomorph"
    assert len(situation.cards()) == 6


def test_everything_may_be_left_out() -> None:
    situation = parse_situation({})
    assert situation.board is None and situation.free_slots() is None and situation.cards() == []
    assert parse_situation({"board": []}).free_slots() == 10
    # A size not known: no free-slot count.
    assert parse_situation({"board": [{"name": "Mystery"}]}).free_slots() is None


def test_sizes_ignore_case() -> None:
    assert parse_situation({"board": [{"name": "A", "size": "large"}]}).board[0].size == "Large"


@pytest.mark.parametrize(
    ("situation", "message"),
    [
        ([], "Input should be a valid dictionary"),
        ({"heroo": "Jules"}, "heroo: Extra inputs are not permitted"),
        ({"day": 0}, "day: Input should be greater than or equal to 1"),
        ({"gold": "lots"}, "gold: Input should be a valid integer"),
        ({"board": [{"name": ""}]}, "board.0.name"),
        ({"board": [{"name": "A", "size": "Huge"}]}, "board.0.size"),
        ({"board": [{"name": "A", "slot": 10}]}, "board.0.slot"),
        ({"offer": [{"name": "A"}]}, "offer.0.kind: Field required"),
        ({"offer": [{"name": "A", "kind": "gift"}]}, "offer.0.kind"),
        ({"opponent": {"board": [{"id": "x"}]}}, "opponent.board.0.name: Field required"),
    ],
)
def test_bad_situations_are_refused_naming_the_field(situation: object, message: str) -> None:
    with pytest.raises(SituationError, match="The situation isn't valid") as error:
        parse_situation(situation)
    assert message in str(error.value)


def test_offer_kinds() -> None:
    assert OFFER_KINDS == ("merchant_item", "loot", "encounter_choice", "skill")
    for kind in OFFER_KINDS:
        parse_situation({"offer": [{"name": "A", "kind": kind}]})


def test_rendering() -> None:
    text = render_situation(parse_situation(SITUATION))
    assert text.splitlines()[:4] == [
        "## Situation",
        "",
        "Hero: Jules · Day 5, Hour 2 · Shop",
        "Gold 12 · Level 4 · Health 900/1200",
    ]
    assert "Board (4 of 10 slots used, 6 free):" in text
    assert "- slot 1: Giant Lollipop (Large, Silver) [id TPL-GIANT-LOLLIPOP]" in text
    assert "Stash: empty." in text and "Skills: empty." in text
    assert "- Laurel's Bouncy Ball (merchant item, Small, Gold) [id TPL-BOUNCY-BALL]" in text
    assert "Opponent" not in text

    sparse = render_situation(parse_situation({"hero": "Dooley", "opponent": {"hero": "Mak", "board": [{"name": "Vial"}]}}))
    assert "Board: not sent." in sparse and "Offer: not sent." in sparse
    assert "Opponent: Mak" in sparse and "- Vial" in sparse


def test_system_prompt() -> None:
    assert system_prompt(None).startswith(PROMPT) and "The app sent no situation" in system_prompt(None)
    assert "Hero: Jules" in system_prompt(SITUATION)
    # An invalid situation still runs: the model is told what was wrong.
    assert "offer.0.kind: Field required" in system_prompt({"offer": [{"name": "A"}]})


def test_the_prompt_rules() -> None:
    for rule in (
        "with its n",
        "fewer than 10 runs",
        "call suggest",
        "call shop_plan",
        "call arrangement",
        "call similar_runs",
        "follow_ups",
        "ask_user only when the question is unclear",
    ):
        assert rule in PROMPT
