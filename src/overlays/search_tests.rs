use super::*;

#[test]
fn letters_must_appear_in_order() {
    assert!(matches("cmo", "Catppuccin Mocha"));
    assert!(!matches("omc", "Catppuccin Mocha"));
}

#[test]
fn case_accents_and_spaces_are_ignored() {
    assert!(matches("ROSE dawn", "Rosé Pine Dawn"));
    assert!(matches("", "anything"));
}
