use super::*;
use crate::theme::{CATPPUCCIN_LATTE, CATPPUCCIN_MOCHA, ROSE_PINE, THEMES};

#[test]
fn opens_on_the_current_theme() {
    assert_eq!(ThemePicker::new(&CATPPUCCIN_MOCHA).selected, 1);
    assert_eq!(ThemePicker::new(&CATPPUCCIN_LATTE).selected, 8);
}

#[test]
fn moving_the_highlight_previews_each_theme() {
    let mut picker = ThemePicker::new(&ROSE_PINE);
    assert_eq!(picker.press("down", None), Outcome::Preview(&CATPPUCCIN_MOCHA));
    assert_eq!(picker.press("up", None), Outcome::Preview(&ROSE_PINE));
}

#[test]
fn up_and_down_wrap_around() {
    let mut picker = ThemePicker::new(&ROSE_PINE);
    assert_eq!(picker.press("up", None), Outcome::Preview(THEMES.last().unwrap()));
    assert_eq!(picker.press("down", None), Outcome::Preview(&ROSE_PINE));
}

#[test]
fn typing_filters_and_previews_the_first_match() {
    let mut picker = ThemePicker::new(&ROSE_PINE);
    for letter in "latt".chars() {
        picker.press(&letter.to_string(), Some(&letter.to_string()));
    }
    assert_eq!(picker.press("e", Some("e")), Outcome::Preview(&CATPPUCCIN_LATTE));
    assert_eq!(picker.matches(), [&CATPPUCCIN_LATTE]);
}

#[test]
fn the_picker_filters_by_appearance() {
    let mut picker = ThemePicker::new(&ROSE_PINE);
    for letter in "light".chars() {
        picker.press(&letter.to_string(), Some(&letter.to_string()));
    }
    assert_eq!(picker.matches().len(), 3);
    assert!(picker.matches().iter().all(|theme| !theme.dark));
}

#[test]
fn enter_chooses_the_highlighted_theme() {
    let mut picker = ThemePicker::new(&ROSE_PINE);
    picker.press("down", None);
    assert_eq!(picker.press("enter", None), Outcome::Choose(&CATPPUCCIN_MOCHA));
}

#[test]
fn escape_restores_the_theme_from_before_the_picker_opened() {
    let mut picker = ThemePicker::new(&CATPPUCCIN_MOCHA);
    picker.press("down", None);
    picker.press("down", None);
    assert_eq!(picker.press("escape", None), Outcome::Restore(&CATPPUCCIN_MOCHA));
}

#[test]
fn a_query_with_no_matches_previews_and_chooses_nothing() {
    let mut picker = ThemePicker::new(&ROSE_PINE);
    assert_eq!(picker.press("z", Some("z")), Outcome::None);
    assert_eq!(picker.press("down", None), Outcome::None);
    assert_eq!(picker.press("enter", None), Outcome::None);
    assert_eq!(picker.press("escape", None), Outcome::Restore(&ROSE_PINE));
}

#[test]
fn backspace_widens_the_filter_and_previews_the_first_theme() {
    let mut picker = ThemePicker::new(&ROSE_PINE);
    picker.press("n", Some("n"));
    picker.press("o", Some("o"));
    picker.press("backspace", None);
    assert_eq!(picker.press("backspace", None), Outcome::Preview(&ROSE_PINE));
    assert_eq!(picker.query, "");
}
