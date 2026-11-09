use std::sync::Arc;
use std::time::Duration;

use bazaar_tracker::catalog::CardCatalog;
use bazaar_tracker::memory::AttachDetails;
use bazaar_tracker::run::PresentedOption;

use super::*;
use crate::tracking::test_events::*;

const CARDS: &str = r#"{ "Items": [
    { "Id": "tpl-pasta", "Localization": { "Title": { "Text": "Pasta Maker" } } },
    { "Id": "tpl-fang", "Localization": { "Title": { "Text": "Fang" } } },
    { "Id": "tpl-katana", "Localization": { "Title": { "Text": "Katana" } } }
] }"#;

fn names() -> Names {
    Names::new(Arc::new(CardCatalog::from_cards_json(CARDS).unwrap()))
}

fn say(narrator: &mut Narrator, notice: Notice) -> LogRow {
    narrator.row(&notice, &names(), 7).expect("a row")
}

fn saved(narrator: &mut Narrator, event: Event) -> String {
    say(narrator, Notice::EventSaved(event)).text
}

fn pasta(size: &str, buy: i32, sell: i32) -> Item {
    let mut item = item("itm_pasta", size, Some("Socket_0"));
    item.template_id = Some("tpl-pasta".into());
    item.tier = "Bronze".into();
    item.attributes.insert("BuyPrice".into(), buy);
    item.attributes.insert("SellPrice".into(), sell);
    item
}

fn offered(id: &str, context: &str, ids: &[&str]) -> Event {
    let options = ids.iter().map(|id| PresentedOption { instance_id: (*id).into(), template_id: Some(format!("tpl-{id}")), kind: "Item".into(), ..PresentedOption::default() }).collect();
    event(id, 0, EventKind::OptionsPresented { context: context.into(), options, snapshot: Snapshot::default() })
}

fn chosen(options_event: Option<&str>, snapshot: Snapshot) -> Event {
    event(
        "sel",
        0,
        EventKind::Selected {
            card_kind: "Item".into(),
            instance_id: "itm_pasta".into(),
            template_id: Some("tpl-pasta".into()),
            tier: Some("Bronze".into()),
            enchantment: None,
            section: None,
            target: None,
            merged_into: None,
            options_event_id: options_event.map(str::to_owned),
            snapshot,
        },
    )
}

#[test]
fn connecting_names_the_game_and_its_unity_version() {
    let details = AttachDetails { unity_version: "6000.3.11".into(), ..AttachDetails::default() };
    let row = say(&mut Narrator::default(), Notice::Connection(ConnectionChange::Connected { pid: 11776, unity_version: "Unity 6 x64".into(), details }));
    assert_eq!((row.kind, row.text.as_str(), row.time_ms), (Kind::Info, "Connected to The Bazaar (pid 11776, Unity 6000.3.11)", 7));
}

#[test]
fn connecting_without_details_falls_back_to_the_table_name() {
    let row = say(&mut Narrator::default(), Notice::Connection(ConnectionChange::Connected { pid: 1, unity_version: "Unity 6 x64".into(), details: AttachDetails::default() }));
    assert_eq!(row.text, "Connected to The Bazaar (pid 1, Unity Unity 6 x64)");
}

#[test]
fn losing_the_game_is_a_warning() {
    let row = say(&mut Narrator::default(), Notice::Connection(ConnectionChange::Disconnected { reason: "the game process exited".into() }));
    assert_eq!((row.kind, row.text.as_str()), (Kind::Warn, "Lost the game: the game process exited"));
}

#[test]
fn problems_are_errors_in_plain_words() {
    let mut narrator = Narrator::default();
    let failed = say(&mut narrator, Notice::SaveFailed { what: "event e1 (Sold)".into(), error: "disk full".into(), retry_in: Duration::from_secs(4) });
    assert_eq!((failed.kind, failed.text.as_str()), (Kind::Error, "Could not save event e1 (Sold): disk full; trying again in 4s"));
    let log = say(&mut narrator, Notice::LogError("access denied".into()));
    assert_eq!((log.kind, log.text.as_str()), (Kind::Error, "Could not read Player.log: access denied"));
}

#[test]
fn a_rebinding_is_an_info_row() {
    let row = say(&mut Narrator::default(), Notice::RunRebound { old_run_id: "local-1".into(), new_run_id: "abc".into() });
    assert_eq!(row.kind, Kind::Info);
    assert!(!row.text.contains("local-1"), "ids are not part of the sentence: {}", row.text);
}

#[test]
fn an_event_row_has_the_time_of_the_event() {
    let row = say(&mut Narrator::default(), Notice::EventSaved(event("e", 1234, EventKind::Rerolled)));
    assert_eq!((row.kind, row.time_ms, row.text.as_str()), (Kind::Event, 1234, "Refreshed the offers"));
}

#[test]
fn a_shop_purchase_says_what_it_cost() {
    let mut narrator = Narrator::default();
    saved(&mut narrator, offered("shop", "EncounterState", &["pasta", "fang"]));
    let snapshot = Snapshot { board: vec![pasta("Medium", 4, 2)], ..Snapshot::default() };
    assert_eq!(saved(&mut narrator, chosen(Some("shop"), snapshot)), "Bought Pasta Maker (Bronze, Medium) for 4 gold");
}

#[test]
fn a_free_choice_is_taken_not_bought() {
    let mut narrator = Narrator::default();
    saved(&mut narrator, offered("loot", "ChoiceState", &["pasta"]));
    let snapshot = Snapshot { board: vec![pasta("Medium", 4, 2)], ..Snapshot::default() };
    assert_eq!(saved(&mut narrator, chosen(Some("loot"), snapshot)), "Took Pasta Maker (Bronze, Medium)");
}

#[test]
fn a_selection_that_is_not_in_the_snapshot_still_reads_well() {
    let text = saved(&mut Narrator::default(), chosen(None, Snapshot::default()));
    assert_eq!(text, "Took Pasta Maker (Bronze)");
}

#[test]
fn a_skill_is_learned() {
    let mut skill = chosen(None, Snapshot::default());
    if let EventKind::Selected { card_kind, .. } = &mut skill.kind {
        *card_kind = "Skill".into();
    }
    assert_eq!(saved(&mut Narrator::default(), skill), "Learned the skill Pasta Maker (Bronze)");
}

#[test]
fn other_choices_are_named_by_their_kind() {
    let mut encounter = chosen(None, Snapshot::default());
    if let EventKind::Selected { card_kind, .. } = &mut encounter.kind {
        *card_kind = "Encounter".into();
    }
    assert_eq!(saved(&mut Narrator::default(), encounter), "Chose encounter Pasta Maker");
}

#[test]
fn a_sale_uses_what_the_player_had_before() {
    let mut narrator = Narrator::default();
    saved(&mut narrator, options("o", 0, Snapshot { board: vec![pasta("Medium", 4, 2)], ..Snapshot::default() }));
    let mut sale = sold("s", 0, Snapshot::default());
    if let EventKind::Sold { instance_id, template_id, tier, .. } = &mut sale.kind {
        *instance_id = "itm_pasta".into();
        *template_id = Some("tpl-pasta".into());
        *tier = Some("Bronze".into());
    }
    assert_eq!(saved(&mut narrator, sale), "Sold Pasta Maker (Bronze, Medium) for 2 gold");
}

#[test]
fn a_sale_of_something_unseen_has_no_price() {
    assert_eq!(saved(&mut Narrator::default(), sold("s", 0, Snapshot::default())), "Sold itm_x");
}

#[test]
fn offers_list_their_names_and_cut_a_long_list() {
    let mut narrator = Narrator::default();
    assert_eq!(saved(&mut narrator, offered("o", "ChoiceState", &["pasta", "fang", "katana"])), "Offered 3 options (Choice): Pasta Maker, Fang, Katana");
    assert_eq!(saved(&mut narrator, offered("o", "LootState", &["pasta"])), "Offered 1 option (Loot): Pasta Maker");
    assert_eq!(saved(&mut narrator, offered("o", "ChoiceState", &["a", "b", "c", "d", "e", "f"])), "Offered 6 options (Choice): tpl-a, tpl-b, tpl-c, tpl-d and 2 more");
}

#[test]
fn a_run_starting_names_the_hero_and_mode() {
    let start = |hero: Option<&str>, mode: Option<&str>| {
        event("s", 0, EventKind::RunStarted { play_mode: mode.map(str::to_owned), build_id: None, environment: None, hero: hero.map(str::to_owned), rating: None, rank: None, rank_division: None })
    };
    assert_eq!(saved(&mut Narrator::default(), start(Some("Vanessa"), Some("Ranked"))), "Run started: Vanessa, Ranked");
    assert_eq!(saved(&mut Narrator::default(), start(None, None)), "Run started");
}

fn fight(id: &str, pvp: bool, name: Option<&str>, hero: Option<&str>) -> Event {
    event(
        id,
        0,
        EventKind::CombatStart {
            player: Snapshot::default(),
            opponent: Snapshot::default(),
            is_pvp: pvp,
            opponent_name: name.map(str::to_owned),
            opponent_hero: hero.map(str::to_owned),
            opponent_rating: None,
            opponent_level: None,
        },
    )
}

fn over(from: Option<&str>, outcome: Outcome) -> Event {
    let mut end = combat_end("end", 0, outcome);
    if let EventKind::CombatEnd { combat_start_event_id, .. } = &mut end.kind {
        *combat_start_event_id = from.map(str::to_owned);
    }
    end
}

#[test]
fn a_combat_says_who_it_was_against_and_how_it_went() {
    let mut narrator = Narrator::default();
    assert_eq!(saved(&mut narrator, fight("c1", true, Some("Mak"), Some("Mak"))), "Combat started: PvP against Mak");
    assert_eq!(saved(&mut narrator, over(Some("c1"), Outcome::Win)), "Combat ended: won against Mak (PvP)");
    assert_eq!(saved(&mut narrator, fight("c2", true, None, Some("Jules"))), "Combat started: PvP against Jules");
    assert_eq!(saved(&mut narrator, over(Some("c2"), Outcome::Loss)), "Combat ended: lost against Jules (PvP)");
    assert_eq!(saved(&mut narrator, fight("c3", true, None, None)), "Combat started: PvP");
    assert_eq!(saved(&mut narrator, over(Some("c3"), Outcome::Win)), "Combat ended: won (PvP)");
    assert_eq!(saved(&mut narrator, fight("c4", false, None, None)), "Combat started: PvE");
    assert_eq!(saved(&mut narrator, over(Some("c4"), Outcome::Win)), "Combat ended: won (PvE)");
}

#[test]
fn a_combat_end_without_its_start_or_result_is_still_a_sentence() {
    let mut narrator = Narrator::default();
    assert_eq!(saved(&mut narrator, over(Some("missing"), Outcome::Loss)), "Combat ended: lost");
    assert_eq!(saved(&mut narrator, over(None, Outcome::Unknown)), "Combat ended: the result was not recorded");
}

#[test]
fn a_run_ending_gives_the_record() {
    let end = |outcome, victories, losses| {
        event(
            "e",
            0,
            EventKind::RunEnded {
                outcome,
                victories,
                losses,
                gold: None,
                health_max: None,
                prestige: None,
                level: None,
                income: None,
                rating_after: None,
                rank_after: None,
                rank_division_after: None,
            },
        )
    };
    assert_eq!(saved(&mut Narrator::default(), end(Outcome::Win, Some(10), Some(2))), "Run ended: victory (10 wins, 2 losses)");
    assert_eq!(saved(&mut Narrator::default(), end(Outcome::Loss, Some(1), Some(1))), "Run ended: defeat (1 win, 1 loss)");
    assert_eq!(saved(&mut Narrator::default(), end(Outcome::Unknown, None, None)), "Run ended: no result recorded");
}

#[test]
fn no_row_looks_like_debug_output() {
    let mut narrator = Narrator::default();
    let events = [
        offered("o", "ChoiceState", &["pasta"]),
        chosen(Some("o"), Snapshot::default()),
        sold("s", 0, Snapshot::default()),
        fight("c", true, Some("Mak"), None),
        over(Some("c"), Outcome::Win),
    ];
    for event in events {
        let text = saved(&mut narrator, event);
        assert!(!text.contains('{') && !text.contains("Some(") && !text.contains("\\\""), "{text}");
    }
}

fn live(state: &str, offered: &[&str]) -> Notice {
    let entities = offered
        .iter()
        .map(|id| {
            let mut entity = item(id, "Medium", None);
            entity.template_id = Some(format!("tpl-{}", id.trim_start_matches("itm_")));
            entity.tier = if id.starts_with("itm_") { "Bronze".into() } else { String::new() };
            entity
        })
        .collect();
    Notice::LiveSnapshot(Some(BoardState { run_state: Some(state.into()), selection_set: offered.iter().map(|id| (*id).to_owned()).collect(), entities, ..BoardState::default() }))
}

#[test]
fn a_shop_is_logged_with_its_items_as_a_now_row() {
    let row = say(&mut Narrator::default(), live("Encounter", &["itm_pasta", "itm_fang", "itm_unknown"]));
    assert_eq!(row.kind, Kind::Now);
    assert_eq!(row.time_ms, 7);
    assert_eq!(row.text, "Shop: Pasta Maker (Bronze, Medium), Fang (Bronze, Medium), tpl-unknown (Bronze, Medium)");
}

#[test]
fn an_encounter_choice_is_logged_by_name() {
    let row = say(&mut Narrator::default(), live("Choice", &["enc_a", "katana"]));
    assert_eq!(row.text, "Encounter choice: tpl-enc_a, Katana");
}

#[test]
fn the_same_offer_is_logged_once_and_a_new_one_again() {
    let mut narrator = Narrator::default();
    assert!(narrator.row(&live("Encounter", &["itm_pasta"]), &names(), 7).is_some());
    assert!(narrator.row(&live("Encounter", &["itm_pasta"]), &names(), 8).is_none(), "the board read every second is not repeated");
    assert_eq!(say(&mut narrator, live("Encounter", &["itm_fang"])).text, "Shop: Fang (Bronze, Medium)", "a reroll is a new offer");
    // Leaving the shop and coming back to the same offer logs it again.
    assert!(narrator.row(&live("Combat", &[]), &names(), 9).is_none());
    assert!(narrator.row(&live("Encounter", &["itm_fang"]), &names(), 10).is_some());
}

#[test]
fn other_states_and_no_board_are_not_logged() {
    let mut narrator = Narrator::default();
    assert!(narrator.row(&Notice::LiveSnapshot(None), &names(), 7).is_none());
    assert!(narrator.row(&live("Combat", &["itm_pasta"]), &names(), 7).is_none());
    assert!(narrator.row(&live("Encounter", &[]), &names(), 7).is_none(), "a shop with nothing left is not an offer");
}
