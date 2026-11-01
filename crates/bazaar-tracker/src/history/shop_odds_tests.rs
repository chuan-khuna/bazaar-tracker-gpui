use bazaardb::{DayOdds, ShopOdds, shop_odds::defaults};

use crate::history::History;

fn open() -> (tempfile::TempDir, History) {
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.db")).unwrap();
    (dir, history)
}

#[test]
fn a_new_file_has_no_shop_odds() {
    let (_dir, history) = open();
    let saved = history.shop_odds().unwrap();
    assert!(saved.odds.days.is_empty());
    assert_eq!(saved.saved_ms, None);
}

#[test]
fn seeding_fills_an_empty_table_once() {
    let (_dir, history) = open();
    assert!(history.seed_shop_odds(&defaults(), 10).unwrap());
    assert!(!history.seed_shop_odds(&ShopOdds::default(), 20).unwrap());
    let saved = history.shop_odds().unwrap();
    assert_eq!(saved.odds, defaults());
    assert_eq!(saved.saved_ms, Some(10));
}

#[test]
fn saving_replaces_every_day() {
    let (_dir, history) = open();
    history.save_shop_odds(&defaults(), 10).unwrap();
    let newer = ShopOdds { days: vec![DayOdds { day: 1, samples: 5, tiers: [50.0, 50.0, 0.0, 0.0] }] };
    history.save_shop_odds(&newer, 30).unwrap();
    let saved = history.shop_odds().unwrap();
    assert_eq!(saved.odds, newer);
    assert_eq!(saved.saved_ms, Some(30));
}
