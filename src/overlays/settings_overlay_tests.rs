use super::*;

fn type_text(overlay: &mut SettingsOverlay, text: &str) {
    for c in text.chars() {
        overlay.press(&c.to_string(), Some(&c.to_string()));
    }
}

#[test]
fn escape_closes_the_overlay() {
    assert_eq!(SettingsOverlay::default().press("escape", None), Outcome::Close);
}

#[test]
fn other_keys_change_nothing() {
    let mut overlay = SettingsOverlay::default();
    assert_eq!(overlay.press("enter", None), Outcome::None);
    assert_eq!(overlay.press("a", Some("a")), Outcome::None);
    assert_eq!(overlay.clear_confirm, None);
}

#[test]
fn clear_data_needs_the_word_typed_then_enter() {
    let mut overlay = SettingsOverlay::asking_to_clear();
    type_text(&mut overlay, "delet");
    assert!(!overlay.clear_confirmed());
    assert_eq!(overlay.press("enter", None), Outcome::None);
    type_text(&mut overlay, "e");
    assert!(overlay.clear_confirmed());
    assert_eq!(overlay.press("enter", None), Outcome::ClearData);
}

#[test]
fn the_word_is_taken_in_any_case_and_backspace_takes_a_letter_off() {
    let mut overlay = SettingsOverlay::asking_to_clear();
    type_text(&mut overlay, "DELETEx");
    assert!(!overlay.clear_confirmed());
    overlay.press("backspace", None);
    assert!(overlay.clear_confirmed());
}

#[test]
fn escape_while_asking_stops_asking_and_keeps_the_overlay_open() {
    let mut overlay = SettingsOverlay::asking_to_clear();
    type_text(&mut overlay, "del");
    assert_eq!(overlay.press("escape", None), Outcome::None);
    assert_eq!(overlay.clear_confirm, None);
    assert_eq!(overlay.press("escape", None), Outcome::Close);
}
