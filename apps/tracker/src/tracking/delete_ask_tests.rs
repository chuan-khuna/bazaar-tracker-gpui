use super::*;

#[test]
fn only_one_row_asks_at_a_time() {
    let mut ask = DeleteAsk::default();
    assert_eq!(ask.asking(), None);
    ask.ask("a");
    assert!(ask.is("a"));
    ask.ask("b");
    assert!(ask.is("b") && !ask.is("a"));
}

#[test]
fn cancel_closes_the_strip_and_says_whether_one_was_open() {
    let mut ask = DeleteAsk::default();
    assert!(!ask.cancel());
    ask.ask("a");
    assert!(ask.cancel());
    assert_eq!(ask.asking(), None);
}

#[test]
fn clicking_another_row_closes_the_strip_but_clicking_its_own_row_does_not() {
    let mut ask = DeleteAsk::default();
    ask.ask("a");
    ask.clicked("a");
    assert!(ask.is("a"));
    ask.clicked("b");
    assert_eq!(ask.asking(), None);
}

#[test]
fn delete_only_goes_ahead_on_the_row_that_asks() {
    let mut ask = DeleteAsk::default();
    assert!(!ask.confirm("a"));
    ask.ask("a");
    assert!(!ask.confirm("b"));
    assert!(ask.is("a"));
    assert!(ask.confirm("a"));
    assert_eq!(ask.asking(), None);
}
