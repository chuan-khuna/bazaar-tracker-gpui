use super::*;

#[test]
fn escape_closes_the_pop_up() {
    assert_eq!(ShopOddsOverlay.press("escape"), Outcome::Close);
}

#[test]
fn other_keys_change_nothing() {
    assert_eq!(ShopOddsOverlay.press("enter"), Outcome::None);
}
