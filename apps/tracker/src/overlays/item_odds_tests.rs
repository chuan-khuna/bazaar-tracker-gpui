use super::*;

fn overlay() -> ItemOddsOverlay {
    ItemOddsOverlay { item: Wanted { template_id: "t".into(), name: "Pasta Maker".into(), hero: "Jules".into(), tier: "Bronze".into() }, day: Some(3) }
}

#[test]
fn escape_closes_the_pop_up() {
    assert_eq!(overlay().press("escape"), Outcome::Close);
}

#[test]
fn other_keys_change_nothing() {
    assert_eq!(overlay().press("enter"), Outcome::None);
}
