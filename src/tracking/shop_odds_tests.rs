use bazaardb::shop_odds::defaults;
use bazaardb::{DayOdds, ShopOdds};

use super::*;

#[test]
fn shows_today_and_the_two_days_after() {
    let shown = upcoming(&defaults(), 4);
    assert_eq!(shown.rows.iter().map(|row| row.day).collect::<Vec<_>>(), vec![4, 5, 6]);
    assert!(shown.rows[0].today && !shown.rows[1].today);
    assert_eq!(shown.rows[2].tiers, Some([1.0, 91.0, 8.0, 0.0]));
    assert_eq!(shown.note, None);
}

#[test]
fn days_past_the_last_borrow_its_odds_and_say_so() {
    let shown = upcoming(&defaults(), 15);
    assert_eq!(shown.rows.iter().map(|row| (row.day, row.borrowed_from)).collect::<Vec<_>>(), vec![(15, None), (16, None), (17, Some(16))]);
    assert_eq!(shown.rows[2].tiers, defaults().for_day(16).map(|d| d.tiers));
    assert_eq!(shown.note.as_deref(), Some("Shop odds only have 16 days; later days show day 16."));
}

#[test]
fn a_gap_in_the_data_has_no_odds() {
    let odds = ShopOdds {
        days: vec![DayOdds { day: 1, samples: 1, tiers: [100.0, 0.0, 0.0, 0.0] }, DayOdds { day: 3, samples: 1, tiers: [0.0, 100.0, 0.0, 0.0] }],
    };
    let shown = upcoming(&odds, 1);
    assert_eq!(shown.rows[1].tiers, None);
    assert_eq!(shown.rows[2].borrowed_from, None);
}

#[test]
fn no_odds_at_all_shows_empty_rows() {
    let shown = upcoming(&ShopOdds::default(), 2);
    assert!(shown.rows.iter().all(|row| row.tiers.is_none()));
    assert_eq!(shown.note, None);
}

#[test]
fn percents_round_and_slivers_show() {
    assert_eq!(percent(0.0), "0%");
    assert_eq!(percent(0.2), "<1%");
    assert_eq!(percent(68.6), "69%");
}
