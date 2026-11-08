use super::*;

const BODY: &str = r#"{
  "itemName": "Pasta Maker", "itemUrlId": "qnwx879f", "hero": "Jules", "tierOffered": "Silver",
  "days": ["1", "2", "3+"], "ownedDataByDay": [true, true, false],
  "pAnyRerollByDay": [0.15, 0.3, 0.4],
  "merchants": [ { "name": "Chris", "alwaysTier": "Silver", "days": [ { "pOfferReroll": 0.05 }, { "pOfferReroll": 0.125 }, { "pOfferReroll": 0.5 } ] } ]
}"#;

fn saved(tier: &str) -> SavedItemOdds {
    SavedItemOdds { template_id: "e45d3e01".into(), hero: "Jules".into(), tier: tier.into(), url_id: "qnwx879f".into(), item_name: "Pasta Maker".into(), body: BODY.into(), saved_ms: 0 }
}

fn wanted() -> Wanted {
    Wanted { template_id: "E45D3E01".into(), name: "Pasta Maker".into(), hero: "Jules".into(), tier: "Bronze".into() }
}

fn reply(item: &str, hero: &str, tier: &str, body: &str) -> bazaardb::ItemOdds {
    bazaardb::ItemOdds { item: item.into(), hero: hero.into(), tier: tier.into(), body: body.into() }
}

#[test]
fn the_book_finds_odds_by_item_hero_and_tier() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    assert!(book.get("E45D3E01", "Jules", "Bronze").is_some());
    assert!(book.get("e45d3e01", "Jules", "Silver").is_none());
    assert!(book.get("e45d3e01", "Vanessa", "Bronze").is_none());
    assert_eq!(book.url_id("E45D3E01"), Some("qnwx879f"));
    assert_eq!(book.url_id("other"), None);
}

#[test]
fn unreadable_saved_odds_are_left_out() {
    let book = ItemOddsBook::load(vec![SavedItemOdds { body: "not json".into(), ..saved("Bronze") }, saved("Silver")]);
    assert_eq!(book.len(), 1);
}

#[test]
fn a_reply_for_the_item_asked_about_is_accepted() {
    let accepted = accept(&wanted(), &reply("qnwx879f", "Jules", "Bronze", BODY), None, 42).unwrap();
    assert_eq!(accepted.saved, SavedItemOdds { tier: "Bronze".into(), saved_ms: 42, ..saved("Bronze") });
    assert_eq!(accepted.note, None);
}

#[test]
fn a_reply_for_another_item_is_refused() {
    let other = BODY.replace("Pasta Maker", "Cheese Wheel").replace("qnwx879f", "zzz");
    let error = accept(&wanted(), &reply("zzz", "Jules", "Bronze", &other), None, 0).unwrap_err();
    assert!(error.contains("Cheese Wheel") && error.contains("Pasta Maker"), "{error}");
}

#[test]
fn a_known_bazaar_db_id_is_enough_when_the_names_differ() {
    let other = BODY.replace("Pasta Maker", "Pasta Machine");
    assert!(accept(&wanted(), &reply("qnwx879f", "Jules", "Bronze", &other), Some("qnwx879f"), 0).is_ok());
}

#[test]
fn odds_for_another_hero_or_tier_are_saved_under_those_with_a_note() {
    let body = BODY.replace("\"Jules\"", "\"Vanessa\"");
    let accepted = accept(&wanted(), &reply("qnwx879f", "Vanessa", "", &body), None, 0).unwrap();
    assert_eq!((accepted.saved.hero.as_str(), accepted.saved.tier.as_str()), ("Vanessa", NOT_OWNED));
    let note = accepted.note.unwrap();
    assert!(note.contains("Vanessa") && note.contains("not owned"), "{note}");
}

#[test]
fn chances_print_as_the_page_does() {
    assert_eq!(percent(0.125), "12.5%");
    assert_eq!(percent(0.3), "30%");
    assert_eq!(percent(0.0525), "5.25%");
    assert_eq!(percent(0.05), "5%");
    assert_eq!(percent(0.), "0%");
}

#[test]
fn the_table_has_any_shop_then_each_merchant_and_marks_the_day() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    let view = table_view(book.get("e45d3e01", "Jules", "Bronze").unwrap(), Some(7));
    assert_eq!(view.title, "Pasta Maker");
    assert_eq!(view.facts, "Jules · owned Bronze · offered at Silver");
    assert_eq!(view.columns, ["Day 1", "2", "3+"]);
    assert_eq!(view.today, Some(2));
    assert_eq!(view.rows[0].name, "Any shop");
    assert_eq!(view.rows[0].cells, [Some("15%".to_owned()), Some("30%".to_owned()), None]);
    assert_eq!(view.rows[1].note.as_deref(), Some("always Silver"));
    assert_eq!(view.rows[1].cells[1].as_deref(), Some("12.5%"));
}

fn tiers_of(choices: &[TierChoice]) -> Vec<(&str, &str, bool)> {
    choices.iter().map(|choice| (choice.tier.as_str(), choice.label.as_str(), choice.saved)).collect()
}

#[test]
fn the_tier_choices_are_not_owned_then_the_tiers_the_item_comes_in() {
    let body = BODY.replace("\"tierOffered\"", "\"itemTiers\": [\"Silver\", \"Gold\"], \"tierOffered\"");
    let book = ItemOddsBook::load(vec![SavedItemOdds { body, ..saved("Silver") }]);
    let choices = tier_choices(&book, &Wanted { tier: NOT_OWNED.into(), ..wanted() });
    assert_eq!(tiers_of(&choices), [(NOT_OWNED, "Not owned", false), ("Silver", "Silver", true), ("Gold", "Gold", false)]);
}

#[test]
fn without_the_items_tiers_every_tier_is_a_choice_and_the_one_asked_about_is_kept() {
    let book = ItemOddsBook::load(vec![saved(NOT_OWNED)]);
    let choices = tier_choices(&book, &Wanted { tier: "Legendary".into(), ..wanted() });
    assert_eq!(tiers_of(&choices), [(NOT_OWNED, "Not owned", true), ("Bronze", "Bronze", false), ("Silver", "Silver", false), ("Gold", "Gold", false), ("Diamond", "Diamond", false), ("Legendary", "Legendary", false)]);
}

#[test]
fn the_missing_tiers_are_the_items_tiers_with_no_odds_saved() {
    let body = BODY.replace("\"tierOffered\"", "\"itemTiers\": [\"Bronze\", \"Silver\", \"Gold\"], \"tierOffered\"");
    let book = ItemOddsBook::load(vec![SavedItemOdds { body, ..saved("Silver") }]);
    let missing: Vec<String> = missing_tiers(&book, &Wanted { tier: "Silver".into(), ..wanted() }).into_iter().map(|wanted| wanted.tier).collect();
    assert_eq!(missing, [NOT_OWNED, "Bronze", "Gold"]);
}

fn chance(chance: &str) -> Chance {
    Chance { template_id: "e45d3e01".into(), name: "Pasta Maker".into(), tier: "Bronze".into(), tier_mark: None, chance: chance.into() }
}

fn pasta(tier: &str) -> Owned {
    Owned { template_id: "E45D3E01".into(), tier: tier.into(), name: "Pasta Maker".into() }
}

#[test]
fn an_encounter_choice_groups_the_items_by_merchant_with_any_shop_last() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    let watched = [pasta("Bronze"), Owned { template_id: "other".into(), tier: "Bronze".into(), name: "Other".into() }];
    let groups = choice_groups(&book, "Jules", &watched, &["Chris".to_owned(), "Kina".to_owned()], 2, true);
    assert_eq!(
        groups,
        [
            MerchantGroup { merchant: "Chris".into(), items: vec![chance("12.5%")] },
            MerchantGroup { merchant: "Kina".into(), items: vec![] },
            MerchantGroup { merchant: "Any shop".into(), items: vec![chance("30%")] },
        ]
    );
}

#[test]
fn nothing_shows_without_odds_for_the_tier_owned_or_the_hero() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    assert!(choice_groups(&book, "Jules", &[pasta("Silver")], &["Chris".to_owned()], 1, true).is_empty());
    assert!(choice_groups(&book, "Vanessa", &[pasta("Bronze")], &["Chris".to_owned()], 1, true).is_empty());
}

#[test]
fn a_tracked_item_is_watched_as_not_owned_unless_it_is_owned() {
    let tracked = [("e45d3e01".to_owned(), "Pasta Maker".to_owned()), ("tpl-wok".to_owned(), "Wok".to_owned())];
    let watched = watched(&[pasta("Bronze")], &tracked, &[]);
    assert_eq!(watched, [pasta("Bronze"), Owned { template_id: "tpl-wok".into(), tier: NOT_OWNED.into(), name: "Wok".into() }]);
}

#[test]
fn a_tracked_item_shows_with_its_not_owned_odds() {
    let book = ItemOddsBook::load(vec![saved(NOT_OWNED)]);
    let watched = watched(&[], &[("e45d3e01".to_owned(), "Pasta Maker".to_owned())], &[]);
    let groups = choice_groups(&book, "Jules", &watched, &["Chris".to_owned()], 1, true);
    assert_eq!(groups.iter().map(|group| group.merchant.as_str()).collect::<Vec<_>>(), ["Chris", "Any shop"]);
}

#[test]
fn any_shop_is_left_out_unless_asked_for() {
    let book = ItemOddsBook::load(vec![saved(NOT_OWNED)]);
    let watched = watched(&[], &[("e45d3e01".to_owned(), "Pasta Maker".to_owned())], &[]);
    let groups = choice_groups(&book, "Jules", &watched, &["Chris".to_owned()], 1, false);
    assert_eq!(groups.iter().map(|group| group.merchant.as_str()).collect::<Vec<_>>(), ["Chris"]);
}

#[test]
fn a_hidden_item_is_not_watched_whether_owned_or_tracked() {
    let tracked = [("tpl-wok".to_owned(), "Wok".to_owned())];
    assert!(watched(&[pasta("Bronze")], &tracked, &["e45d3e01".to_owned(), "TPL-WOK".to_owned()]).is_empty());
}

fn items() -> Vec<(String, String)> {
    vec![("tpl-a".into(), "Anvil".into()), ("tpl-p".into(), "Pasta Maker".into()), ("tpl-w".into(), "Pasta Wok".into())]
}

#[test]
fn the_tracked_items_come_first_then_matches_for_the_query() {
    let rows = candidates(&items(), &["tpl-w".to_owned(), "gone".to_owned()], &[], "pasta", 10);
    let shown: Vec<_> = rows.iter().map(|row| (row.name.as_str(), row.tracked)).collect();
    assert_eq!(shown, [("Pasta Wok", true), ("gone", true), ("Pasta Maker", false)]);
}

#[test]
fn without_a_query_only_the_tracked_items_are_listed_and_matches_are_capped() {
    assert_eq!(candidates(&items(), &["tpl-a".to_owned()], &[], "", 10).len(), 1);
    assert_eq!(candidates(&items(), &[], &[], "a", 2).len(), 2);
    assert_eq!(candidates(&items(), &[], &[], "pastamaker", 10)[0].template_id, "tpl-p");
}

fn catalog_item(id: &str, name: &str, heroes: &[&str]) -> CatalogItem {
    CatalogItem { template_id: id.into(), name: name.into(), heroes: heroes.iter().map(|hero| (*hero).to_owned()).collect() }
}

fn catalog() -> Vec<CatalogItem> {
    vec![
        catalog_item("e45d3e01", "Pasta Maker", &["Jules"]),
        catalog_item("tpl-k", "Knife", &["Vanessa", "Jules"]),
        catalog_item("tpl-r", "Rock", &[NEUTRAL_HERO]),
        catalog_item("tpl-g", "Gear", &["Dooley"]),
    ]
}

#[test]
fn the_heroes_are_the_ones_items_name_without_neutral() {
    assert_eq!(heroes(&catalog()), ["Dooley", "Jules", "Vanessa"]);
}

#[test]
fn the_queue_has_each_heros_items_then_the_neutral_ones() {
    let book = ItemOddsBook::default();
    let names = |queue: Vec<Wanted>| queue.into_iter().map(|wanted| format!("{}:{}", wanted.hero, wanted.name)).collect::<Vec<_>>();
    assert_eq!(names(load_queue(&catalog(), &["Jules".into(), "Dooley".into()], true, &book)), ["Jules:Pasta Maker", "Jules:Knife", "Jules:Rock", "Dooley:Gear", "Dooley:Rock"]);
    assert_eq!(names(load_queue(&catalog(), &["Vanessa".into()], false, &book)), ["Vanessa:Knife"]);
    assert!(load_queue(&catalog(), &[], true, &book).is_empty());
    assert!(load_queue(&catalog(), &["Vanessa".into()], true, &book).iter().all(|wanted| wanted.tier == NOT_OWNED));
}

#[test]
fn the_queue_leaves_out_items_with_odds_saved_for_the_hero() {
    let book = ItemOddsBook::load(vec![saved(NOT_OWNED)]);
    let queue = load_queue(&catalog(), &["Jules".into()], false, &book);
    assert_eq!(queue.iter().map(|wanted| wanted.name.as_str()).collect::<Vec<_>>(), ["Knife"]);
}

#[test]
fn hidden_items_come_after_the_tracked_ones_and_leave_the_matches() {
    let rows = candidates(&items(), &["tpl-w".to_owned(), "tpl-a".to_owned()], &["tpl-a".to_owned(), "tpl-p".to_owned()], "pasta", 10);
    let shown: Vec<_> = rows.iter().map(|row| (row.name.as_str(), row.tracked, row.hidden)).collect();
    assert_eq!(shown, [("Pasta Wok", true, false), ("Anvil", true, true), ("Pasta Maker", false, true)]);
}

#[test]
fn an_item_owned_at_two_tiers_shows_each_marked_with_its_tier() {
    let book = ItemOddsBook::load(vec![saved("Bronze"), saved("Silver")]);
    let watched = watched(&[pasta("Bronze"), pasta("Silver"), pasta("Bronze")], &[], &[]);
    assert_eq!(watched, [pasta("Bronze"), pasta("Silver")]);
    let groups = choice_groups(&book, "Jules", &watched, &[], 1, true);
    let marks: Vec<_> = groups[0].items.iter().map(|chance| chance.tier_mark.as_deref()).collect();
    assert_eq!(marks, [Some("B"), Some("S")]);
}

#[test]
fn the_best_shops_are_the_merchants_most_likely_to_have_an_item_that_day() {
    let body = BODY.replace(
        r#"{ "name": "Chris", "alwaysTier": "Silver", "days": [ { "pOfferReroll": 0.05 }, { "pOfferReroll": 0.125 }, { "pOfferReroll": 0.5 } ] }"#,
        r#"{ "name": "Chris", "days": [ { "pOfferReroll": 0.05 }, { "pOfferReroll": 0.125 }, { "pOfferReroll": 0.5 } ] },
           { "name": "Kina", "days": [ { "pOfferReroll": 0.3 }, { "pOfferReroll": 0.25 }, { "pOfferReroll": 0.1 } ] },
           { "name": "Aila", "days": [ { "pOfferReroll": 0.2 }, {}, { "pOfferReroll": 0.2 } ] },
           { "name": "Vel", "days": [ { "pOfferReroll": 0.01 }, { "pOfferReroll": 0.5 }, { "pOfferReroll": 0.3 } ] }"#,
    );
    let book = ItemOddsBook::load(vec![SavedItemOdds { body, ..saved("Bronze") }]);
    let best = best_shops(&book, "Jules", &[pasta("Bronze")], 1, 3);
    assert_eq!(best.len(), 1);
    let names = |best: &[BestShops]| best[0].shops.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&best), ["Kina", "Aila", "Chris"]);
    // Day 2: Aila has no chance there, so the three are the others.
    assert_eq!(names(&best_shops(&book, "Jules", &[pasta("Bronze")], 2, 3)), ["Vel", "Kina", "Chris"]);
    assert!(best_shops(&book, "Vanessa", &[pasta("Bronze")], 1, 3).is_empty());
}

#[test]
fn a_merchant_carrying_none_of_the_watched_items_has_an_empty_group() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    let groups = choice_groups(&book, "Jules", &[pasta("Bronze")], &["Laurel".to_owned()], 1, false);
    assert_eq!(groups, [MerchantGroup { merchant: "Laurel".into(), items: vec![] }]);
}

fn chose(id: &str, kind: &str, template: &str) -> Event {
    Event {
        id: id.into(),
        run_id: "run".into(),
        timestamp_ms: 0,
        day: None,
        hour: None,
        kind: EventKind::Selected {
            card_kind: kind.into(),
            instance_id: format!("inst-{id}"),
            template_id: Some(template.into()),
            tier: None,
            enchantment: None,
            section: None,
            target: None,
            merged_into: None,
            options_event_id: None,
            snapshot: Default::default(),
        },
    }
}

#[test]
fn the_current_merchant_is_the_encounter_chosen_last() {
    let names = Names::default();
    assert_eq!(current_merchant(&[], &names), None);
    let events = [chose("1", "Encounter", "kina"), chose("2", "Item", "pasta"), chose("3", "Encounter", "herma"), chose("4", "Item", "wok")];
    assert_eq!(current_merchant(&events, &names), Some(names.card(Some("herma"), "inst-3")));
}

#[test]
fn a_choice_grid_has_a_column_for_each_merchant_and_a_row_for_each_item() {
    let cell = |chance: &str| Some(GridCell { merchant: None, chance: chance.into() });
    let wok = Chance { template_id: "wok".into(), name: "Wok".into(), tier: NOT_OWNED.into(), tier_mark: None, chance: "4%".into() };
    let groups = [
        MerchantGroup { merchant: "Chris".into(), items: vec![chance("12.5%")] },
        MerchantGroup { merchant: "Laurel".into(), items: vec![] },
        MerchantGroup { merchant: "Kina".into(), items: vec![wok, chance("3%")] },
    ];
    let grid = choice_grid(&groups);
    assert_eq!(grid.columns, ["Chris", "Laurel", "Kina"]);
    let rows: Vec<_> = grid.rows.iter().map(|row| (row.name.as_str(), row.cells.clone())).collect();
    assert_eq!(rows, [("Pasta Maker", vec![cell("12.5%"), None, cell("3%")]), ("Wok", vec![None, None, cell("4%")])]);
}

#[test]
fn a_best_grid_has_a_column_for_each_place() {
    let best = [BestShops { template_id: "pasta".into(), name: "Pasta Maker".into(), tier: NOT_OWNED.into(), tier_mark: None, shops: vec![("Kina".into(), "30%".into()), ("Aila".into(), "20%".into())] }];
    let grid = best_grid(&best, 3);
    assert_eq!(grid.columns, ["1st", "2nd", "3rd"]);
    assert_eq!(grid.rows[0].cells, [Some(GridCell { merchant: Some("Kina".into()), chance: "30%".into() }), Some(GridCell { merchant: Some("Aila".into()), chance: "20%".into() }), None]);
}

#[test]
fn heroes_missing_from_the_hero_list_are_named_once() {
    let catalog = ["Vanessa".to_owned(), "Hero8".to_owned(), "Hero9".to_owned()];
    assert_eq!(unknown_heroes(&catalog, Some("Hero9")), ["Hero9"]);
    assert_eq!(unknown_heroes(&catalog, Some("Hero10")), ["Hero10", "Hero9"]);
    assert!(unknown_heroes(&catalog[..2], None).is_empty());
}

#[test]
fn the_filter_picks_the_board_the_stash_the_tracked_or_all() {
    let wok = Owned { template_id: "tpl-wok".into(), tier: "Silver".into(), name: "Wok".into() };
    let tracked = [("tpl-pan".to_owned(), "Pan".to_owned())];
    let pan = Owned { template_id: "tpl-pan".into(), tier: NOT_OWNED.into(), name: "Pan".into() };
    let (board, stash) = ([pasta("Bronze")], [wok.clone()]);
    assert_eq!(watched_by(OddsFilter::All, &board, &stash, &tracked, &[]), [pasta("Bronze"), wok.clone(), pan.clone()]);
    assert_eq!(watched_by(OddsFilter::Board, &board, &stash, &tracked, &[]), [pasta("Bronze")]);
    assert_eq!(watched_by(OddsFilter::Stash, &board, &stash, &tracked, &[]), [wok.clone()]);
    assert_eq!(watched_by(OddsFilter::Tracked, &board, &stash, &tracked, &[]), [pan]);
    assert_eq!(watched_by(OddsFilter::Stash, &board, &stash, &tracked, &["TPL-WOK".to_owned()]), []);
}

fn not_owned(template_id: &str, name: &str) -> Owned {
    Owned { template_id: template_id.into(), tier: NOT_OWNED.into(), name: name.into() }
}

#[test]
fn a_search_keeps_the_watched_items_that_match_then_adds_other_items_the_hero_can_find() {
    let watched = vec![pasta("Bronze"), not_owned("tpl-wok", "Wok")];
    // "k" is in Pasta Maker, Wok, Knife (Jules's too) and Rock (neutral, so anyone's).
    assert_eq!(search(watched.clone(), &catalog(), "Jules", "K"), [pasta("Bronze"), not_owned("tpl-wok", "Wok"), not_owned("tpl-k", "Knife"), not_owned("tpl-r", "Rock")]);
    // "r" is in Pasta Maker, Rock (neutral, so anyone's) and Gear (Dooley's, so not Jules's);
    // the watched Pasta Maker is not added again.
    assert_eq!(search(watched.clone(), &catalog(), "Jules", "r"), [pasta("Bronze"), not_owned("tpl-r", "Rock")]);
}

#[test]
fn an_empty_search_changes_nothing() {
    let watched = vec![pasta("Bronze")];
    assert_eq!(search(watched.clone(), &catalog(), "Jules", "  "), watched);
}

#[test]
fn the_items_without_saved_odds_are_the_ones_to_load() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    let items = [pasta("Bronze"), not_owned("tpl-k", "Knife")];
    let wanted = without_odds(&items, &book, "Jules");
    assert_eq!(wanted, [Wanted { template_id: "tpl-k".into(), name: "Knife".into(), hero: "Jules".into(), tier: NOT_OWNED.into() }]);
}
