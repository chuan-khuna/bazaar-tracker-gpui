//! The JSON here is hand-written in the shape the page's script reads (`days[].day`, `.samples`,
//! `.tiers[<Tier>]`); the numbers are made up, not Bazaar DB's.

use super::*;

const ANSWER: &str = r#"{"days":[
  {"day":2,"samples":900,"tiers":{"Bronze":60,"Silver":35.5,"Gold":4.5}},
  {"day":1,"samples":1000,"tiers":{"Bronze":100}},
  {"day":5,"samples":12,"tiers":{"Bronze":10,"Silver":40,"Gold":40,"Diamond":10}}
]}"#;

#[test]
fn reads_each_day_in_order() {
    let odds = parse_json(ANSWER).unwrap();
    assert_eq!(odds.days.iter().map(|d| d.day).collect::<Vec<_>>(), vec![1, 2, 5]);
    assert_eq!(odds.days[1], DayOdds { day: 2, samples: 900, tiers: [60.0, 35.5, 4.5, 0.0] });
}

#[test]
fn a_missing_tier_is_zero_and_a_missing_day_is_none() {
    let odds = parse_json(ANSWER).unwrap();
    assert_eq!(odds.for_day(1).unwrap().tiers, [100.0, 0.0, 0.0, 0.0]);
    assert!(odds.for_day(3).is_none());
    assert_eq!(odds.last_day(), Some(5));
}

#[test]
fn an_empty_answer_has_no_days() {
    assert_eq!(parse_json("{}").unwrap(), ShopOdds::default());
}

#[test]
fn a_refusal_or_a_challenge_says_so() {
    assert!(matches!(parse_json("Unauthorized"), Err(ParseError::Blocked)));
    assert!(matches!(parse_json("<title>Just a moment...</title>"), Err(ParseError::Blocked)));
}

#[test]
fn the_defaults_cover_days_1_to_16_and_each_adds_to_100_give_or_take_rounding() {
    let odds = defaults();
    assert_eq!(odds.days.iter().map(|d| d.day).collect::<Vec<_>>(), (1..=16).collect::<Vec<_>>());
    for day in &odds.days {
        assert!((day.tiers.iter().sum::<f32>() - 100.0).abs() <= 1.0, "day {}", day.day);
    }
    assert_eq!(odds.for_day(6).unwrap().tiers, [1.0, 91.0, 8.0, 0.0]);
}

#[test]
fn other_text_is_a_json_error() {
    assert!(matches!(parse_json("<html></html>"), Err(ParseError::Json(_))));
}
