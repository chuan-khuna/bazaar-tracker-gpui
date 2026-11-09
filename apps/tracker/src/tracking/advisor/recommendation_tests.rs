use serde_json::{Value, json};

use super::*;

/// `suggest`'s result on the record's example: Jules, Giant Lollipop and Pizza Cutter on the Board,
/// the Offer ranked Bouncy Ball, Decanter, Cheese Wheel; plus a fourth card with no data. The rows
/// come out of order, to check the card sorts them.
fn record_example() -> Value {
    json!({
        "title": "Shop · Day 5",
        "hero": "Jules",
        "n": 15,
        "seasons": [{"season": 20, "runs": 15}],
        "sample": "S20 (15 runs)",
        "thin": false,
        "source": "15 Jules runs · S20 · 1 profile",
        "profiles": 1,
        "ranked": [
            {
                "rank": 2, "card": "Decanter", "id": "dec", "kind": "item", "size": "medium", "tier": "SILVER", "fits": false, "no_data": false,
                "runs_with": {"n": 5, "of": 15, "run_ids": ["r1", "r2", "r3", "r4", "r5"]},
                "picks": {"offered": 16, "taken": 9, "take_rate": 0.56, "offered_run_ids": ["r1"], "taken_run_ids": ["r1"]},
                "with_board": [
                    {"card": "Giant Lollipop", "id": "gl", "n": 2, "of": 5, "lift": 1.2, "win_gain": null, "run_ids": ["r1", "r2"]},
                    {"card": "Pizza Cutter", "id": "pc", "n": 3, "of": 4, "lift": 2.25, "win_gain": null, "run_ids": ["r1", "r3", "r4"]}
                ],
                "reason": "Close behind with Pizza Cutter"
            },
            {
                "rank": 1, "card": "Laurel's Bouncy Ball", "id": "lbb", "kind": "item", "size": "Small", "tier": "Bronze", "fits": true, "no_data": false,
                "runs_with": {"n": 4, "of": 15, "run_ids": ["r1", "r3", "r6", "r7"]},
                "picks": {"offered": 15, "taken": 7, "take_rate": 0.47,
                          "offered_run_ids": ["r1", "r1", "r3", "r6", "r8"], "taken_run_ids": ["r1", "r3"]},
                "with_board": [
                    {"card": "Giant Lollipop", "id": "gl", "n": 3, "of": 5, "lift": 2.25, "win_gain": null, "run_ids": ["r1", "r3", "r6"]},
                    {"card": "Pizza Cutter", "id": "pc", "n": 3, "of": 4, "lift": 2.81, "win_gain": null, "run_ids": ["r1", "r3", "r7"]}
                ],
                "reason": "Shows up with both your items"
            },
            {
                "rank": 4, "card": "Mystery Box", "id": "mb", "kind": "skill", "size": null, "tier": null, "fits": null, "no_data": true,
                "runs_with": {"n": 0, "of": 15, "run_ids": []},
                "picks": {"offered": 0, "taken": 0, "take_rate": null, "offered_run_ids": [], "taken_run_ids": []},
                "with_board": [],
                "reason": "From its card text: scales late"
            },
            {
                "rank": 3, "card": "Cheese Wheel", "id": "cw", "kind": "item", "size": "Medium", "tier": "Bronze", "fits": true, "no_data": false,
                "runs_with": {"n": 4, "of": 15, "run_ids": ["r2"]},
                "picks": {"offered": 15, "taken": 11, "take_rate": 0.73, "offered_run_ids": ["r2"], "taken_run_ids": []},
                "with_board": [
                    {"card": "Giant Lollipop", "id": "gl", "n": 1, "of": 5, "lift": null, "win_gain": null, "run_ids": ["r2"]},
                    {"card": "Pizza Cutter", "id": "pc", "n": 2, "of": 4, "lift": null, "win_gain": null, "run_ids": ["r2"]}
                ],
                "reason": "Taken a lot, but rarely with your items"
            }
        ],
        "runs": {
            "r1": {
                "run_id": "r1", "hero": "Jules", "season": 20, "wins": 10, "losses": 3, "victory_tier": "Gold Victory",
                "url": "https://bazaardb.gg/run/tracker/r1",
                "board": [
                    {"id": "gl", "name": "Giant Lollipop", "tier": "Silver", "slot": 1, "size": 3},
                    {"id": "unknown-card", "name": "Pizza Cutter", "tier": "Gold", "slot": 4, "size": 1, "enchantment": "Fiery"}
                ],
                "skills": [{"id": "sk", "name": "Sharp Eye", "tier": "Gold"}]
            },
            "r3": {"run_id": "r3", "hero": "Jules", "season": 20, "wins": 6, "losses": 10, "victory_tier": null, "url": "https://example.com/r3", "board": [], "skills": []}
        }
    })
}

fn arguments() -> Value {
    json!({"title": "Shop · Day 5", "ranked": [{"card": "Laurel's Bouncy Ball", "reason": "Shows up with both your items"}, {"card": "Decanter", "reason": "Close behind"}]})
}

fn card() -> Recommendation {
    Recommendation::of(&arguments(), Some(&record_example()), false).unwrap()
}

fn bars(row: &Row) -> &[Bar] {
    match &row.data {
        RowData::Bars(bars) => bars,
        other => panic!("no bars: {other:?}"),
    }
}

#[test]
fn rows_are_ranked_best_first_with_size_tier_and_reason() {
    let card = card();
    assert_eq!(card.title, "Shop · Day 5");
    assert!(!card.pending);
    let names: Vec<(usize, &str)> = card.rows.iter().map(|row| (row.rank, row.card.as_str())).collect();
    assert_eq!(names, [(1, "Laurel's Bouncy Ball"), (2, "Decanter"), (3, "Cheese Wheel"), (4, "Mystery Box")]);
    let ball = &card.rows[0];
    assert_eq!((ball.id.as_deref(), ball.size.as_deref(), ball.tier.as_deref()), (Some("lbb"), Some("Small"), Some("Bronze")));
    assert_eq!(ball.reason, "Shows up with both your items");
    // Sizes and tiers in any case read as the game names them.
    assert_eq!((card.rows[1].size.as_deref(), card.rows[1].tier.as_deref()), (Some("Medium"), Some("Silver")));
    assert_eq!((card.rows[3].size.as_deref(), card.rows[3].tier.as_deref()), (None, None));
}

#[test]
fn fits_doesnt_fit_and_unknown() {
    let card = card();
    let fits: Vec<Fit> = card.rows.iter().map(|row| row.fit).collect();
    assert_eq!(fits, [Fit::Yes, Fit::No, Fit::Yes, Fit::Unknown]);
}

#[test]
fn the_board_bar_is_the_best_lift_and_taken_when_offered_counts_picks() {
    let card = card();
    let ball = bars(&card.rows[0]);
    assert_eq!(ball.len(), 2);
    assert_eq!(ball[0].label, "with your Board");
    assert_eq!(ball[0].count(), "3/4");
    assert_eq!(ball[0].note.as_deref(), Some("with Pizza Cutter · lift 2.81"));
    assert!((ball[0].fraction() - 0.75).abs() < 1e-6);
    assert_eq!(ball[1].label, "taken when offered");
    assert_eq!(ball[1].count(), "7/15");
    // An offer seen twice in one run lists the run once; the runs it was taken in are marked.
    let offered: Vec<(&str, bool)> = ball[1].runs.iter().map(|run| (run.id.as_str(), run.taken)).collect();
    assert_eq!(offered, [("r1", true), ("r3", true), ("r6", false), ("r8", false)]);
    // Without a lift on either pair, the most runs wins.
    let wheel = bars(&card.rows[2]);
    assert_eq!((wheel[0].count(), wheel[0].note.as_deref()), ("2/4".to_owned(), Some("with Pizza Cutter")));
}

#[test]
fn an_empty_board_shows_runs_on_final_boards_instead() {
    let mut result = record_example();
    result["ranked"][1]["with_board"] = json!([]);
    let card = Recommendation::of(&arguments(), Some(&result), false).unwrap();
    let ball = bars(&card.rows[0]);
    assert_eq!((ball[0].label.as_str(), ball[0].count()), ("on final Boards", "4/15".to_owned()));
    assert_eq!(ball[0].runs.len(), 4);
}

#[test]
fn a_row_with_no_data_says_so() {
    let card = card();
    assert_eq!(card.rows[3].data, RowData::NoData);
    assert!(card.rows[3].run_ids().is_empty());
    assert_eq!(NO_DATA, "no data · from card text");
}

#[test]
fn a_row_lists_the_runs_behind_its_bars_once() {
    let card = card();
    assert_eq!(card.rows[0].run_ids(), ["r1", "r3", "r7", "r6", "r8"]);
}

#[test]
fn the_source_line_and_thin_data() {
    let card = card();
    assert_eq!(card.source, Some(Source { text: "15 Jules runs · S20 · 1 profile".into(), thin: false }));

    let mut thin = record_example();
    thin["n"] = json!(6);
    thin["thin"] = json!(true);
    thin["source"] = json!("6 Jules runs · S20 · 1 profile · thin ⚠");
    let card = Recommendation::of(&arguments(), Some(&thin), false).unwrap();
    assert_eq!(card.source, Some(Source { text: "6 Jules runs · S20 · 1 profile · thin ⚠".into(), thin: true }));

    // With no source line in the result, one is made from its numbers, and ⚠ is added when thin.
    let mut bare = thin.clone();
    bare.as_object_mut().unwrap().remove("source");
    bare["sample"] = json!("S19 + S20 (6 runs)");
    let card = Recommendation::of(&arguments(), Some(&bare), false).unwrap();
    assert_eq!(card.source.unwrap().text, "6 Jules runs · S19 + S20 (6 runs) · 1 profile · thin ⚠");
    // A thin flag missing: under 10 runs is thin.
    let mut unflagged = bare;
    unflagged.as_object_mut().unwrap().remove("thin");
    unflagged["source"] = json!("6 Jules runs · S20 · 1 profile");
    let card = Recommendation::of(&arguments(), Some(&unflagged), false).unwrap();
    assert_eq!(card.source, Some(Source { text: "6 Jules runs · S20 · 1 profile · thin ⚠".into(), thin: true }));
}

#[test]
fn while_suggest_runs_the_card_is_drawn_from_its_arguments() {
    let card = Recommendation::of(&arguments(), None, true).unwrap();
    assert!(card.pending);
    assert_eq!(card.rows.len(), 2);
    assert_eq!(card.rows[1].card, "Decanter");
    assert!(card.rows.iter().all(|row| row.data == RowData::Waiting && row.fit == Fit::Unknown));
    assert_eq!(card.source, None);
    assert!(card.runs.is_empty());
    // Arguments without `ranked` make no card.
    assert_eq!(Recommendation::of(&json!({"title": "x"}), None, true), None);
}

#[test]
fn source_runs_with_their_result_board_and_link() {
    let card = card();
    let r1 = &card.runs["r1"];
    assert_eq!(r1.result(), "S20 · 10–3 · Gold Victory");
    assert_eq!(r1.link(), Some("https://bazaardb.gg/run/tracker/r1"));
    assert_eq!(r1.board[1], RunCard { id: "unknown-card".into(), name: "Pizza Cutter".into(), tier: Some("Gold".into()), slot: Some(4), size: Some(1), enchantment: Some("Fiery".into()) });
    let r3 = &card.runs["r3"];
    assert_eq!(r3.result(), "S20 · 6–10");
    // Only Bazaar DB pages are opened.
    assert_eq!(r3.link(), None);
    assert_eq!(SourceRun::default().result(), "result not known");
    // The hover lists only the runs the result carries.
    let taken = &bars(&card.rows[0])[1];
    let listed: Vec<(&str, bool)> = card.runs_of(taken).map(|(run, taken)| (run.id.as_str(), taken)).collect();
    assert_eq!(listed, [("r1", true), ("r3", true)]);
}

#[test]
fn a_source_runs_board_is_laid_out_by_the_board_renderer() {
    let card = card();
    let names = Names::default();
    let grid = card.runs["r1"].board_grid(&names);
    let tiles: Vec<(u8, u8, &str)> = grid.tiles.iter().map(|tile| (tile.start, tile.span, tile.name.as_str())).collect();
    // Each on its own slot and size; names the catalog doesn't know are the run's.
    assert_eq!(tiles, [(1, 3, "Giant Lollipop"), (4, 1, "Pizza Cutter")]);
    assert_eq!(grid.tiles[1].enchantment.as_deref(), Some("Fiery"));
    assert_eq!(grid.tiles[0].tier, Some(board::Tier::Silver));
    let skills = card.runs["r1"].skill_chips(&names);
    assert_eq!(skills.len(), 1);
    assert_eq!((skills[0].name.as_str(), skills[0].tier), ("Sharp Eye", Some(board::Tier::Gold)));
}

#[test]
fn the_popup_stays_while_the_pointer_is_on_the_count_or_the_popup() {
    let mut hover = Hover::default();
    hover.count("a", true);
    assert_eq!(hover.shown(), Some("a"));
    // Moving onto the popup: both change in one move.
    hover.count("a", false);
    hover.popup("a", true);
    assert_eq!(hover.shown(), Some("a"));
    hover.popup("a", false);
    assert_eq!(hover.shown(), None);
    // Leaving one count doesn't close another's popup.
    hover.count("b", true);
    hover.count("a", false);
    assert_eq!(hover.shown(), Some("b"));
}
