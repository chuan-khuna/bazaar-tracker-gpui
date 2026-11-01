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

fn pasta(tier: &str) -> Owned {
    Owned { template_id: "E45D3E01".into(), tier: tier.into(), name: "Pasta Maker".into() }
}

#[test]
fn an_encounter_choice_groups_the_items_by_merchant_with_any_shop_last() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    let watched = [pasta("Bronze"), Owned { template_id: "other".into(), tier: "Bronze".into(), name: "Other".into() }];
    let groups = choice_groups(&book, "Jules", &watched, &["Chris".to_owned(), "Kina".to_owned()], 2);
    assert_eq!(
        groups,
        [
            MerchantGroup { merchant: "Chris".into(), items: vec![("Pasta Maker".into(), "12.5%".into())] },
            MerchantGroup { merchant: "Any shop".into(), items: vec![("Pasta Maker".into(), "30%".into())] },
        ]
    );
}

#[test]
fn nothing_shows_without_odds_for_the_tier_owned_or_the_hero() {
    let book = ItemOddsBook::load(vec![saved("Bronze")]);
    assert!(choice_groups(&book, "Jules", &[pasta("Silver")], &["Chris".to_owned()], 1).is_empty());
    assert!(choice_groups(&book, "Vanessa", &[pasta("Bronze")], &["Chris".to_owned()], 1).is_empty());
}

#[test]
fn a_tracked_item_is_watched_as_not_owned_unless_it_is_owned() {
    let tracked = [("e45d3e01".to_owned(), "Pasta Maker".to_owned()), ("tpl-wok".to_owned(), "Wok".to_owned())];
    let watched = watched(&[pasta("Bronze")], &tracked);
    assert_eq!(watched, [pasta("Bronze"), Owned { template_id: "tpl-wok".into(), tier: NOT_OWNED.into(), name: "Wok".into() }]);
}

#[test]
fn a_tracked_item_shows_with_its_not_owned_odds() {
    let book = ItemOddsBook::load(vec![saved(NOT_OWNED)]);
    let watched = watched(&[], &[("e45d3e01".to_owned(), "Pasta Maker".to_owned())]);
    let groups = choice_groups(&book, "Jules", &watched, &["Chris".to_owned()], 1);
    assert_eq!(groups.iter().map(|group| group.merchant.as_str()).collect::<Vec<_>>(), ["Chris", "Any shop"]);
}

fn items() -> Vec<(String, String)> {
    vec![("tpl-a".into(), "Anvil".into()), ("tpl-p".into(), "Pasta Maker".into()), ("tpl-w".into(), "Pasta Wok".into())]
}

#[test]
fn the_tracked_items_come_first_then_matches_for_the_query() {
    let rows = candidates(&items(), &["tpl-w".to_owned(), "gone".to_owned()], "pasta", 10);
    let shown: Vec<_> = rows.iter().map(|row| (row.name.as_str(), row.tracked)).collect();
    assert_eq!(shown, [("Pasta Wok", true), ("gone", true), ("Pasta Maker", false)]);
}

#[test]
fn without_a_query_only_the_tracked_items_are_listed_and_matches_are_capped() {
    assert_eq!(candidates(&items(), &["tpl-a".to_owned()], "", 10).len(), 1);
    assert_eq!(candidates(&items(), &[], "a", 2).len(), 2);
    assert_eq!(candidates(&items(), &[], "pastamaker", 10)[0].template_id, "tpl-p");
}
