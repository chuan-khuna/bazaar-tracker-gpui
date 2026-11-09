"""What the model gets of a Conversation: old questions marked, old situation results left out."""

from __future__ import annotations

from tau_agent import AssistantMessage, TextContent, ToolCall, ToolResultMessage, UserMessage

from agents.advisor.history import model_history

DAY_3 = {"hero": "Jules", "day": 3, "hour": 1, "offer": [{"name": "Cutlass", "kind": "merchant_item"}]}
DAY_5 = {"hero": "Jules", "day": 5, "hour": 2, "offer": [{"name": "Pizza Cutter", "kind": "merchant_item"}]}


def turn(question: str, situation: object | None, tool: str) -> list[tuple[object, object | None]]:
    call = ToolCall(id=f"call-{tool}", name=tool, arguments={})
    return [
        (UserMessage(content=question), situation),
        (AssistantMessage(content=[call], stop_reason="toolUse"), None),
        (ToolResultMessage(tool_call_id=call.id, tool_name=tool, content=[TextContent(text='{"n": 7}')], details={"n": 7}), None),
        (AssistantMessage(content="Take the Cutlass.", stop_reason="stop"), None),
    ]


def test_an_earlier_situation_is_marked_and_its_results_left_out() -> None:
    history = model_history(turn("Which?", DAY_3, "rate_options"), DAY_5)

    assert history[0].text == "[Asked on Day 3, Hour 1, with a different situation from now.]\n\nWhich?"
    result = history[2]
    assert isinstance(result, ToolResultMessage) and result.tool_call_id == "call-rate_options"
    assert "earlier situation (on Day 3, Hour 1" in result.text and "Call rate_options again" in result.text
    assert result.details is None
    # The call and the answer stay, so every call still has its result.
    assert history[1].tool_calls[0].name == "rate_options"
    assert history[3].text == "Take the Cutlass."


def test_the_same_situation_keeps_everything() -> None:
    stored = turn("Which?", DAY_3, "rate_options")
    assert model_history(stored, dict(DAY_3)) == [message for message, _ in stored]


def test_results_that_dont_read_the_situation_are_kept() -> None:
    history = model_history(turn("What does Cutlass do?", DAY_3, "find_card"), DAY_5)
    assert history[2].text == '{"n": 7}'


def test_a_question_without_a_situation() -> None:
    history = model_history(turn("Which?", None, "suggest"), DAY_5)
    assert history[0].text.startswith("[Asked with no situation.]")
    assert "left out" in history[2].text
    assert model_history(turn("Which?", None, "suggest"), None)[2].text == '{"n": 7}'


def test_an_event_question_says_which_event() -> None:
    event = DAY_3 | {"event": {"id": "e-4", "kind": "Picked", "number": 4}}
    assert model_history(turn("Why?", event, "synergy"), DAY_5)[0].text.startswith("[Asked about Event #4 on Day 3")
