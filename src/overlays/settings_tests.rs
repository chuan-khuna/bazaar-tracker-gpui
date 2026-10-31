use super::*;

#[test]
fn escape_closes_the_overlay() {
    assert_eq!(SettingsOverlay.press("escape"), Outcome::Close);
}

#[test]
fn other_keys_change_nothing() {
    assert_eq!(SettingsOverlay.press("enter"), Outcome::None);
    assert_eq!(SettingsOverlay.press("a"), Outcome::None);
}
