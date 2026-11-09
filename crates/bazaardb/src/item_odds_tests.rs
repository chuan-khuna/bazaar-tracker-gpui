use super::*;

const ANSWER: &str = r#"{
  "itemName": "Pasta Maker", "itemUrlId": "qnwx879f", "hero": "Jules", "tierOffered": "Bronze",
  "itemTiers": ["Bronze", "Silver"], "heroOptions": ["Jules"], "itemHeroes": ["Jules"],
  "days": ["1", 2, "3+"], "ownedDataByDay": [true, true, false],
  "pAnyByDay": [0.1, 0.2, 0.0], "pAnyRerollByDay": [0.15, 0.3, null],
  "merchants": [
    { "name": "Chris's Cookware", "uri": "/card/x", "alwaysTier": null,
      "days": [ { "pOfferReroll": 0.05, "pSeen": 0.4 }, { "pOfferReroll": 0.25, "pSeen": null }, { "pOfferReroll": 0.5 } ] }
  ]
}"#;

#[test]
fn the_answer_is_read() {
    let table = parse_json(ANSWER).unwrap();
    assert_eq!((table.item_name.as_str(), table.item_url_id.as_str(), table.hero.as_str()), ("Pasta Maker", "qnwx879f", "Jules"));
    assert_eq!(table.tier_offered.as_deref(), Some("Bronze"));
    assert_eq!(table.item_tiers, ["Bronze", "Silver"]);
    assert_eq!(table.days, ["1", "2", "3+"]);
    assert_eq!(table.measured, [true, true, false]);
    assert_eq!(table.any_reroll_by_day, [Some(0.15), Some(0.3), None]);
    assert_eq!(table.merchants[0].days[0], MerchantDay { offer_reroll: Some(0.05), seen: Some(0.4) });
}

#[test]
fn a_day_past_the_last_column_falls_in_its_plus_column() {
    let table = parse_json(ANSWER).unwrap();
    assert_eq!(table.column_of(2), Some(1));
    assert_eq!(table.column_of(3), Some(2));
    assert_eq!(table.column_of(9), Some(2));
    assert_eq!(table.column_of(0), None);
}

#[test]
fn chances_are_found_by_merchant_name_and_hidden_where_unmeasured() {
    let table = parse_json(ANSWER).unwrap();
    assert_eq!(table.merchant_chance("chris's cookware", 1), Some(0.25));
    assert_eq!(table.merchant_chance("Chriss Cookware", 0), Some(0.05));
    assert_eq!(table.merchant_chance("Chris's Cookware", 2), None);
    assert_eq!(table.merchant_chance("Someone else", 0), None);
    assert_eq!(table.any_chance(0), Some(0.15));
    assert_eq!(table.any_chance(2), None);
}

#[test]
fn missing_fields_leave_an_empty_table() {
    let table = parse_json("{}").unwrap();
    assert!(table.days.is_empty() && table.merchants.is_empty());
    assert!(table.is_measured(0));
    assert!(parse_json("not json").is_err());
}

#[test]
fn names_match_without_case_or_punctuation() {
    assert_eq!(name_key("Pasta Maker"), name_key("pasta-maker"));
    assert_ne!(name_key("Pasta Maker"), name_key("Pasta Makers"));
}
