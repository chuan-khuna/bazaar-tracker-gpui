"""shop_plan, arrangement and similar_runs, and the ``knowledge`` queries behind them, on the record's
example (``scraped_runs.py``) plus, for shop_plan, saved shop and item odds and three runs that chose
encounters on day 5."""

from __future__ import annotations

import json
import sqlite3
from dataclasses import replace
from pathlib import Path

import pytest
from sqlalchemy import text
from scraped_runs import BALL, CATALOG, CUTTER, LOLLIPOP, SITUATION, run_document, selection, write_record_example, write_run
from test_tools import call, tools

import knowledge
from agents.advisor.sources import Sources
from backend.db import open_history
from importer import import_builds
from knowledge.cards import CardCatalog, CardInfo
from knowledge.odds import read_item_odds

CHRIS = "TPL-CHRIS"
PIGGLES = "TPL-PIGGLES"
FIGHT = "TPL-FIGHT"
STEP = "TPL-STEP"

PLAN_CATALOG = CardCatalog(
    [
        *CATALOG,
        CardInfo(id=CHRIS, name="Chris", card_type="PedestalEncounter", heroes=("Common",)),
        CardInfo(id=PIGGLES, name="Piggles", card_type="PedestalEncounter", heroes=("Common",)),
        CardInfo(id=FIGHT, name="Bandit", card_type="CombatEncounter", heroes=("Common",)),
        CardInfo(id=STEP, name="Take the Gold", card_type="EncounterStep", heroes=("Common",)),
    ]
)


def merchant(name: str, offer: list[float], seen: list[float], always_tier: str | None = None) -> dict:
    return {"name": name, "alwaysTier": always_tier, "days": [{"pOfferReroll": o, "pSeen": s} for o, s in zip(offer, seen)]}


def item_body(name: str, any_shop: list[float], merchants: list[dict]) -> str:
    """Bazaar DB's /api/item-odds answer: days 1-5 and 6+, the last not measured."""
    return json.dumps(
        {
            "itemName": name,
            "days": ["1", "2", "3", "4", "5", "6+"],
            "ownedDataByDay": [True] * 5 + [False],
            "pAnyRerollByDay": any_shop,
            "merchants": merchants,
        }
    )


BALL_BODY = item_body(
    "Laurel's Bouncy Ball",
    [0.01, 0.02, 0.05, 0.1, 0.25, 0.3],
    [
        merchant("Chris", [0, 0, 0, 0.05, 0.12, 0.2], [0.3, 0.3, 0.3, 0.4, 0.4, 0.4]),
        merchant("Piggles", [0, 0, 0, 0.1, 0.2, 0.3], [0.2, 0.2, 0.2, 0.3, 0.3, 0.3], "Silver"),
        merchant("Nobody", [0] * 6, [0.1] * 6),
    ],
)


@pytest.fixture
def plan_sources(tmp_path: Path, history_db: Path):
    """The record's 15 Jules runs plus three that chose encounters on day 5 (one with 3 wins, so not
    strong), the encounters in the catalog, and saved shop and item odds."""
    builds = tmp_path / "builds"
    write_record_example(builds)
    write_run(builds, run_document("enc-1", events=[selection(5, 1, [CHRIS, PIGGLES], CHRIS), selection(5, 2, [STEP], STEP)]))
    write_run(builds, run_document("enc-2", wins=12, events=[selection(5, 1, [CHRIS, FIGHT], CHRIS)]))
    write_run(builds, run_document("enc-weak", wins=3, events=[selection(5, 1, [PIGGLES, CHRIS], PIGGLES)]))

    connection = sqlite3.connect(history_db)
    connection.executemany(
        "INSERT INTO imported_shop_odds (day, samples, bronze, silver, gold, diamond, imported_ms, source) VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
        [(1, 120884, 97.0, 3.0, 0, 0, "builtin"), (5, 128989, 25.0, 75.0, 0, 0, "bazaardb"), (10, 1000, 0, 20.0, 70.0, 10.0, "bazaardb")],
    )
    connection.executemany(
        "INSERT INTO imported_item_odds (template_id, hero, tier, url_id, item_name, body, imported_ms, source) VALUES (?, ?, ?, 'u', ?, ?, ?, 'scraper')",
        [
            (BALL.lower(), "Jules", "None", "Laurel's Bouncy Ball", BALL_BODY, 1),
            (CUTTER.lower(), "Jules", "None", "Pizza Cutter", item_body("Pizza Cutter", [0.5] * 6, []), 2),
            (CUTTER.lower(), "Jules", "Silver", "Pizza Cutter", item_body("Pizza Cutter", [0.1] * 6, []), 1),
        ],
    )
    connection.commit()
    connection.close()

    engine = open_history(history_db)
    import_builds(engine, builds)
    yield Sources(engine=engine, catalog=PLAN_CATALOG)
    engine.dispose()


# --- knowledge ----------------------------------------------------------------------------------------


def test_item_odds_columns_and_bad_bodies() -> None:
    day5 = read_item_odds(BALL, "Jules", "None", BALL_BODY, 5, "scraper")
    assert (day5.column, day5.measured, day5.any_shop, day5.item_name) == ("5", True, 0.25, "Laurel's Bouncy Ball")
    # Best chance first; a merchant with no chance is left out.
    assert [(m.merchant, m.chance, m.seen, m.always_tier) for m in day5.merchants] == [("Piggles", 0.2, 0.3, "Silver"), ("Chris", 0.12, 0.4, None)]
    # Day 9 is in the 6+ column, which isn't measured: no chances.
    day9 = read_item_odds(BALL, "Jules", "None", BALL_BODY, 9, "scraper")
    assert (day9.column, day9.measured, day9.any_shop, day9.merchants) == ("6+", False, None, ())
    assert read_item_odds(BALL, "Jules", "None", "{not json", 5, "x").column is None
    assert read_item_odds(BALL, "Jules", "None", json.dumps({"days": [1, 2]}), 2, "x").column == "2"


def test_tier_odds_and_day_picks(plan_sources: Sources) -> None:
    with plan_sources.connection() as connection:
        assert knowledge.tier_odds(connection, 5).percent == {"bronze": 25.0, "silver": 75.0, "gold": 0.0, "diamond": 0.0}
        # A day without its own row takes the last saved day before it.
        assert (knowledge.tier_odds(connection, 7).row_day, knowledge.tier_odds(connection, 12).row_day) == (5, 10)
        picks = knowledge.day_picks(connection, "jules", 5)
    assert (picks.sample.n, len(picks.strong_run_ids)) == (18, 17)
    assert "enc-weak" not in picks.strong_run_ids
    assert [(p.card_id, p.taken, p.offered) for p in picks.picks] == [(CHRIS, 2, 2), (STEP, 1, 1), (FIGHT, 0, 1), (PIGGLES, 0, 1)]


# --- shop_plan --------------------------------------------------------------------------------------------


def test_shop_plan(plan_sources: Sources) -> None:
    result, _ = call(tools(plan_sources)["shop_plan"], {"wanted": ["Laurel's Bouncy Ball", "Pizza Cutter", "Decanter"]})

    assert (result["hero"], result["day"], result["n"], result["seasons"], result["thin"]) == ("Jules", 5, 18, [{"season": 20, "runs": 18}], False)
    assert result["tier_odds"] == {
        "day": 5, "samples": 128989, "bronze_pct": 25.0, "silver_pct": 75.0, "gold_pct": 0.0, "diamond_pct": 0.0, "source": "bazaardb",
    }
    ball, cutter, decanter = result["wanted"]
    assert (ball["owned_tier"], ball["odds_tier"], ball["column"], ball["any_shop_pct"]) == (None, None, "5", 25.0)
    assert ball["merchants"] == [
        {"merchant": "Piggles", "chance_pct": 20.0, "seen_pct": 30.0, "always_tier": "Silver"},
        {"merchant": "Chris", "chance_pct": 12.0, "seen_pct": 40.0, "always_tier": None},
    ]
    # Pizza Cutter is on the Board at Silver: the Silver odds, not the not-owned ones.
    assert (cutter["owned_tier"], cutter["odds_tier"], cutter["any_shop_pct"]) == ("Silver", "Silver", 10.0)
    assert decanter["no_odds"] is True and "any_shop_pct" not in decanter

    encounters = result["encounters"]
    assert (encounters["strong_runs"], encounters["rule"]) == (17, "runs with 10+ wins")
    # The EncounterStep and the item picks are left out; the run with 3 wins doesn't count.
    assert [(e["card"], e["type"], e["taken"], e["offered"], e["run_ids"]) for e in encounters["picks"]] == [
        ("Chris", "PedestalEncounter", 2, 2, ["enc-1", "enc-2"]),
        ("Bandit", "CombatEncounter", 0, 1, []),
        ("Piggles", "PedestalEncounter", 0, 1, []),
    ]


def test_shop_plan_other_days_and_no_data(plan_sources: Sources) -> None:
    result, _ = call(tools(plan_sources, None)["shop_plan"], {"hero": "Jules", "day": 12})
    assert (result["tier_odds"]["day"], result["wanted"], result["encounters"]["picks"]) == (10, [], [])
    # No odds saved and no catalog to tell encounters by: still n and the seasons.
    with plan_sources.connection() as connection:
        connection.execute(text("DELETE FROM imported_shop_odds"))
        connection.commit()
    empty = Sources(engine=plan_sources._engine, catalog=CardCatalog())
    result, _ = call(tools(empty)["shop_plan"], {})
    assert (result["n"], result["tier_odds"], result["encounters"]["picks"]) == (18, None, [])
    assert "No card catalog" in result["encounters"]["note"] and "No shop odds" in result["tier_odds_note"]


def test_shop_plan_refuses_bad_arguments(plan_sources: Sources) -> None:
    plan = tools(plan_sources)["shop_plan"]
    for arguments, message in [
        ({"day": 0}, "1-30"),
        ({"wanted": []}, "1-10"),
        ({"wanted": ["Nope"]}, "No card named"),
        ({"wanted": "Decanter"}, "must be a array"),
    ]:
        with pytest.raises(ValueError, match=message):
            call(plan, arguments)
    with pytest.raises(ValueError, match="No day"):
        call(tools(plan_sources, {"hero": "Jules"})["shop_plan"], {})
    with pytest.raises(ValueError, match="No hero"):
        call(tools(plan_sources, None)["shop_plan"], {"day": 3})


# --- arrangement ------------------------------------------------------------------------------------------


def test_arrangement(sources: Sources) -> None:
    # Pizza Cutter's text cares about position; Giant Lollipop's doesn't.
    cutter = replace(CATALOG.get(CUTTER), description="Adjacent items have +5 Damage.")
    catalog = CardCatalog([cutter if card.id == CUTTER else card for card in CATALOG])
    result, _ = call(tools(Sources(engine=sources._engine, catalog=catalog))["arrangement"], {})

    assert (result["hero"], result["n"], result["seasons"], result["free_slots"]) == ("Jules", 15, [{"season": 20, "runs": 15}], 6)
    # Each record Board starts at slot 0: Giant Lollipop (3 slots) then Pizza Cutter in jules-01 and -02.
    assert result["pairs"] == [
        {"cards": ["Giant Lollipop", "Pizza Cutter"], "ids": [LOLLIPOP, CUTTER], "n": 2, "of": 2, "lift": 1.5, "run_ids": ["jules-01", "jules-02"]}
    ]
    lollipop, pizza = result["positions"]
    assert (lollipop["n"], lollipop["slots"], lollipop["leftmost"], lollipop["rightmost"], lollipop["slot_now"]) == (5, {"0": 5}, 5, 0, 1)
    assert (pizza["n"], pizza["slots"], pizza["leftmost"], pizza["slot_now"], pizza["size"]) == (4, {"0": 2, "3": 2}, 2, 4, "Small")
    assert pizza["run_ids"] == ["jules-01", "jules-02", "jules-04", "jules-07"]
    assert result["position_text"] == [{"card": "Pizza Cutter", "id": CUTTER, "text": "Adjacent items have +5 Damage."}]


def test_arrangement_by_name_and_refusals(sources: Sources) -> None:
    arrange = tools(sources)["arrangement"]
    result, _ = call(arrange, {"board": ["Pizza Cutter", "Laurel's Bouncy Ball", "Cheese Wheel"]})
    pairs = {tuple(pair["cards"]): (pair["n"], pair["of"]) for pair in result["pairs"]}
    # Side by side in jules-01, -02 and -04, the only Boards with both.
    assert pairs[("Pizza Cutter", "Laurel's Bouncy Ball")] == (3, 3)
    assert result["position_text"] == []
    for arguments, message in [
        ({"board": ["Decanter"]}, "2-10"),
        ({"board": ["Decanter", "decanter "]}, "2-10"),
        ({"board": ["Decanter", "Nope"]}, "No card named"),
    ]:
        with pytest.raises(ValueError, match=message):
            call(arrange, arguments)
    with pytest.raises(ValueError, match="fewer than 2"):
        call(tools(sources, SITUATION | {"board": SITUATION["board"][:1]})["arrangement"], {})


# --- similar_runs -----------------------------------------------------------------------------------------


def test_similar_runs(sources: Sources) -> None:
    result, for_model = call(tools(sources)["similar_runs"], {"limit": 3})

    assert (result["hero"], result["n"], result["seasons"], result["cards"]) == ("Jules", 15, [{"season": 20, "runs": 15}], ["Giant Lollipop", "Pizza Cutter"])
    # Both cards in jules-01 and -02 (-02's Board is closer: fewer other cards), then one of them.
    assert [(r["run_id"], r["overlap"], r["of"], r["jaccard"]) for r in result["runs"]] == [
        ("jules-02", 2, 2, 0.5),
        ("jules-01", 2, 2, 0.4),
        ("jules-03", 1, 2, 0.25),
    ]
    best = result["runs"][0]
    assert best["shared"] == ["Giant Lollipop", "Pizza Cutter"]
    assert (best["wins"], best["losses"], best["victory_tier"], best["url"]) == (10, 2, "Gold Victory", "https://bazaardb.gg/run/tracker/jules-02")
    assert [c["name"] for c in best["board"]] == ["Giant Lollipop", "Pizza Cutter", "Laurel's Bouncy Ball", "TPL-FILLER-1"]
    assert best["skills"] == []
    # The model reads it all: the runs are what it answers from.
    assert for_model == result


def test_similar_runs_by_name_and_refusals(sources: Sources) -> None:
    similar = tools(sources)["similar_runs"]
    result, _ = call(similar, {"cards": ["Cheese Wheel"]})
    assert [r["run_id"] for r in result["runs"]] == ["jules-09", "jules-06", "jules-07", "jules-04"]
    # Cutlass is in no Jules run.
    assert call(similar, {"cards": ["Cutlass"]})[0]["runs"] == []
    for arguments, message in [
        ({"cards": []}, "1-10"),
        ({"limit": 11}, "1-10"),
        ({"cards": ["Nope"]}, "No card named"),
    ]:
        with pytest.raises(ValueError, match=message):
            call(similar, arguments)
    with pytest.raises(ValueError, match="no known cards"):
        call(tools(sources, SITUATION | {"board": []})["similar_runs"], {})


# --- an Offer of encounters -------------------------------------------------------------------------------

ENCOUNTER_OFFER = SITUATION | {
    "game_state": "Encounter choice",
    "offer": [
        {"id": CHRIS, "name": "Chris", "kind": "encounter_choice"},
        {"id": PIGGLES, "name": "Piggles", "kind": "encounter_choice"},
        {"id": FIGHT, "name": "Bandit", "kind": "encounter_choice"},
    ],
}


def test_shop_plan_rates_an_offer_of_encounters(plan_sources: Sources) -> None:
    result, _ = call(tools(plan_sources, ENCOUNTER_OFFER)["shop_plan"], {"wanted": ["Laurel's Bouncy Ball"]})
    assert [(o["encounter"], o["strong_runs"] and (o["strong_runs"]["taken"], o["strong_runs"]["offered"]), o["sells_wanted"]) for o in result["offered"]] == [
        ("Chris", (2, 2), [{"card": "Laurel's Bouncy Ball", "chance_pct": 12.0, "always_tier": None}]),
        ("Piggles", (0, 1), [{"card": "Laurel's Bouncy Ball", "chance_pct": 20.0, "always_tier": "Silver"}]),
        ("Bandit", (0, 1), []),
    ]
    assert "offered_note" in result
    # An Offer of cards: no 'offered'.
    assert "offered" not in call(tools(plan_sources)["shop_plan"], {})[0]


def test_rate_options_leaves_encounters_to_shop_plan(plan_sources: Sources) -> None:
    rate = tools(plan_sources, ENCOUNTER_OFFER)["rate_options"]
    with pytest.raises(ValueError, match="not cards: call shop_plan"):
        call(rate, {})
    with pytest.raises(ValueError, match="not cards: call shop_plan"):
        call(rate, {"options": ["Chris"]})
