use crate::history::{History, SavedItemOdds};

fn open() -> (tempfile::TempDir, History) {
    let dir = tempfile::tempdir().unwrap();
    let history = History::open(&dir.path().join("history.db")).unwrap();
    (dir, history)
}

fn odds(tier: &str, body: &str, saved_ms: i64) -> SavedItemOdds {
    SavedItemOdds {
        template_id: "E45D3E01-ED5E".into(),
        hero: "Jules".into(),
        tier: tier.into(),
        url_id: "qnwx879f".into(),
        item_name: "Pasta Maker".into(),
        body: body.into(),
        saved_ms,
    }
}

#[test]
fn a_new_file_has_no_item_odds() {
    let (_dir, history) = open();
    assert!(history.item_odds().unwrap().is_empty());
}

#[test]
fn saved_odds_are_read_back_with_the_id_lower_cased() {
    let (_dir, history) = open();
    history.save_item_odds(&odds("Bronze", "{}", 10)).unwrap();
    let saved = history.item_odds().unwrap();
    assert_eq!(saved, [SavedItemOdds { template_id: "e45d3e01-ed5e".into(), ..odds("Bronze", "{}", 10) }]);
}

#[test]
fn saving_again_replaces_only_the_same_hero_and_tier() {
    let (_dir, history) = open();
    history.save_item_odds(&odds("Bronze", "old", 10)).unwrap();
    history.save_item_odds(&odds("Silver", "silver", 20)).unwrap();
    history.save_item_odds(&odds("Bronze", "new", 30)).unwrap();
    let saved = history.item_odds().unwrap();
    assert_eq!(saved.iter().map(|odds| (odds.tier.as_str(), odds.body.as_str(), odds.saved_ms)).collect::<Vec<_>>(), [("Bronze", "new", 30), ("Silver", "silver", 20)]);
}

#[test]
fn an_import_adds_new_odds_and_replaces_only_older_ones() {
    let (_dir, history) = open();
    history.save_item_odds(&odds("Bronze", "older", 10)).unwrap();
    history.save_item_odds(&odds("Silver", "newer", 50)).unwrap();
    let changed = history.import_item_odds(&[odds("Bronze", "imported", 20), odds("Silver", "imported", 40), odds("Gold", "imported", 30)]).unwrap();
    assert_eq!(changed, 2);
    let bodies: Vec<(String, String)> = history.item_odds().unwrap().into_iter().map(|saved| (saved.tier, saved.body)).collect();
    assert_eq!(bodies, [("Bronze".into(), "imported".into()), ("Gold".into(), "imported".into()), ("Silver".into(), "newer".into())]);
}

#[test]
fn importing_the_same_odds_twice_changes_nothing_the_second_time() {
    let (_dir, history) = open();
    assert_eq!(history.import_item_odds(&[odds("Bronze", "{}", 10)]).unwrap(), 1);
    assert_eq!(history.import_item_odds(&[odds("Bronze", "{}", 10)]).unwrap(), 0);
}
