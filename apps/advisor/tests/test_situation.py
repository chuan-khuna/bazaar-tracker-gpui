"""The situation: checking the app's JSON and rendering it above the question."""

from __future__ import annotations

import pytest
from scraped_runs import SITUATION

from agents.advisor.agent import PROMPT
from agents.advisor.history import question_message
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


def test_the_question_goes_below_its_situation() -> None:
    text = question_message("Which?", SITUATION).text
    assert text.startswith("## Situation for this question\n")
    assert "Hero: Jules" in text and text.endswith("## Question\n\nWhich?")
    assert "The app sent no situation" in question_message("Which?", None).text
    # An invalid situation still runs: the model is told what was wrong.
    assert "offer.0.kind: Field required" in question_message("Which?", {"offer": [{"name": "A"}]}).text


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
        "Answer from the latest question's situation only",
    ):
        assert rule in PROMPT


ATTACHED = {
    "id": "e7",
    "number": 7,
    "kind": "Options",
    "time": "+12:05",
    "detail": "3 options · Choice",
    "picked": {"id": "TPL-BOUNCY-BALL", "name": "Laurel's Bouncy Ball", "size": "Small", "tier": "Gold"},
}


def test_an_attached_event_parses_and_renders_first() -> None:
    situation = parse_situation(SITUATION | {"event": ATTACHED})
    assert situation.event.number == 7 and situation.event.picked.name == "Laurel's Bouncy Ball"
    # The picked card is among the cards the tools can resolve.
    assert situation.cards()[-1].name == "Laurel's Bouncy Ball"
    lines = render_situation(situation).splitlines()
    assert lines[2] == (
        "The player is asking about a past moment: Event #7 (Options: 3 options · Choice) at +12:05 into the run. "
        "Everything below is the game as it was at that Event, not now."
    )
    assert lines[3] == "Picked: Laurel's Bouncy Ball (Small, Gold) [id TPL-BOUNCY-BALL]"
    assert lines[5] == "Hero: Jules · Day 5, Hour 2 · Shop"

    sold = render_situation(parse_situation({"event": {"id": "e8", "kind": "Sold", "sold": {"name": "Anvil"}}}))
    assert "An Event (Sold)." in sold and "Sold: Anvil" in sold
    assert "past moment" not in render_situation(parse_situation(SITUATION))


@pytest.mark.parametrize(
    ("event", "message"),
    [
        ({"kind": "Options"}, "event.id: Field required"),
        ({"id": "e7"}, "event.kind: Field required"),
        ({"id": "e7", "kind": "Options", "number": 0}, "event.number: Input should be greater than or equal to 1"),
        ({"id": "e7", "kind": "Sold", "sold": {"id": "x"}}, "event.sold.name: Field required"),
        ({"id": "e7", "kind": "Sold", "when": "now"}, "event.when: Extra inputs are not permitted"),
    ],
)
def test_bad_events_are_refused_naming_the_field(event: object, message: str) -> None:
    with pytest.raises(SituationError) as error:
        parse_situation({"event": event})
    assert message in str(error.value)


def test_slot_effects_are_rendered_one_line_a_slot_with_the_card_on_it() -> None:
    situation = parse_situation(
        SITUATION
        | {
            "slot_effects": [
                {"slot": 3, "effect": "heat", "name": "Stove"},
                {"slot": 9, "effect": "chill", "name": "Cooler"},
                {"slot": 3, "effect": "note", "name": "B Note (Burn)"},
            ]
        }
    )
    text = render_situation(situation)
    assert "Slot effects:" in text
    # Giant Lollipop (Large, from slot 1) covers slot 3.
    assert "- slot 3: Heat (Stove: the item on it is Heated) + Note (B Note (Burn)); on Giant Lollipop" in text
    assert "- slot 9: Chill (Cooler: the item on it is Chilled); no card on it" in text
    # None sent: no section.
    assert "Slot effects" not in render_situation(parse_situation(SITUATION))
    with pytest.raises(SituationError, match="slot_effects.0.effect"):
        parse_situation({"slot_effects": [{"slot": 1, "effect": "warm", "name": "Stove"}]})


def test_the_opponents_slot_effects() -> None:
    them = {"name": "Rival", "board": [{"name": "Pan", "size": "Large", "slot": 0}], "slot_effects": [{"slot": 1, "effect": "heat", "name": "Stove"}]}
    text = render_situation(parse_situation({"opponent": them}))
    assert "Opponent's slot effects:\n- slot 1: Heat (Stove: the item on it is Heated); on Pan" in text
