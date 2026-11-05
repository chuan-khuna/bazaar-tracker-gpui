use super::*;

fn type_text(overlay: &mut SettingsOverlay, text: &str) {
    for c in text.chars() {
        overlay.press(&c.to_string(), Some(&c.to_string()));
    }
}

fn ask(overlay: &mut SettingsOverlay) -> &mut ClearAsk {
    overlay.clear.as_mut().unwrap()
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
    assert_eq!(overlay.clear, None);
}

#[test]
fn clear_data_asks_about_both_and_needs_the_word_typed_then_enter() {
    let mut overlay = SettingsOverlay::asking_to_clear();
    type_text(&mut overlay, "delet");
    assert!(!ask(&mut overlay).confirmed());
    assert_eq!(overlay.press("enter", None), Outcome::None);
    type_text(&mut overlay, "e");
    assert_eq!(overlay.press("enter", None), Outcome::ClearData { database: true, art: true });
}

#[test]
fn only_what_is_picked_is_cleared_and_nothing_picked_clears_nothing() {
    let mut overlay = SettingsOverlay::asking_to_clear();
    type_text(&mut overlay, "delete");
    ask(&mut overlay).database = false;
    assert_eq!(overlay.press("enter", None), Outcome::ClearData { database: false, art: true });
    ask(&mut overlay).art = false;
    assert!(!ask(&mut overlay).confirmed());
    assert_eq!(overlay.press("enter", None), Outcome::None);
}

#[test]
fn the_word_is_taken_in_any_case_and_backspace_takes_a_letter_off() {
    let mut overlay = SettingsOverlay::asking_to_clear();
    type_text(&mut overlay, "DELETEx");
    assert!(!ask(&mut overlay).confirmed());
    overlay.press("backspace", None);
    assert!(ask(&mut overlay).confirmed());
}

#[test]
fn escape_while_asking_stops_asking_and_keeps_the_overlay_open() {
    let mut overlay = SettingsOverlay::asking_to_clear();
    type_text(&mut overlay, "del");
    assert_eq!(overlay.press("escape", None), Outcome::None);
    assert_eq!(overlay.clear, None);
    assert_eq!(overlay.press("escape", None), Outcome::Close);
}
