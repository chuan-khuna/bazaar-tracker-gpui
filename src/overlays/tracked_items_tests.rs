use super::*;

#[test]
fn typing_filters_and_backspace_takes_it_back() {
    let mut overlay = TrackedItemsOverlay::default();
    overlay.press("p", Some("p"), 0);
    overlay.press("a", Some("a"), 0);
    assert_eq!(overlay.query, "pa");
    overlay.press("backspace", None, 0);
    assert_eq!(overlay.query, "p");
}

#[test]
fn up_and_down_move_around_the_rows_and_enter_toggles_the_highlighted_one() {
    let mut overlay = TrackedItemsOverlay::default();
    overlay.press("down", None, 3);
    overlay.press("down", None, 3);
    assert_eq!(overlay.selected, 2);
    overlay.press("down", None, 3);
    assert_eq!(overlay.selected, 0);
    overlay.press("up", None, 3);
    assert_eq!(overlay.press("enter", None, 3), Outcome::Toggle(2));
}

#[test]
fn typing_goes_back_to_the_first_row_and_enter_on_no_rows_does_nothing() {
    let mut overlay = TrackedItemsOverlay { query: String::new(), selected: 2 };
    overlay.press("w", Some("w"), 3);
    assert_eq!(overlay.selected, 0);
    assert_eq!(overlay.press("enter", None, 0), Outcome::None);
}

#[test]
fn escape_closes_and_control_text_is_ignored() {
    let mut overlay = TrackedItemsOverlay::default();
    overlay.press("tab", Some("\t"), 0);
    assert!(overlay.query.is_empty());
    assert_eq!(overlay.press("escape", None, 0), Outcome::Close);
}
