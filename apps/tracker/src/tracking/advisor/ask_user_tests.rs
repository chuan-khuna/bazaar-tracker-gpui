use serde_json::json;

use super::*;

fn ask(multiple: bool, allow_free_text: bool) -> AskUser {
    AskUser::parse(&json!({
        "question": "Which plan?",
        "options": [{"id": "a", "label": "Weapons"}, {"id": "b", "label": "Shields"}, {"id": "c", "label": "Economy"}],
        "multiple": multiple,
        "allowFreeText": allow_free_text,
    }))
    .unwrap()
}

fn ids(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn the_arguments_are_read_as_tau_app_reads_them() {
    let read = AskUser::parse(&json!({"question": "Q?", "options": [{"id": "a", "label": "A"}, {"id": 2, "label": "bad"}, "bad"]})).unwrap();
    assert_eq!(read, AskUser { question: "Q?".into(), options: vec![Choice { id: "a".into(), label: "A".into() }], multiple: false, allow_free_text: true });
    // Without a question or any good option it is drawn as a tool line instead.
    assert_eq!(AskUser::parse(&json!({"options": [{"id": "a", "label": "A"}]})), None);
    assert_eq!(AskUser::parse(&json!({"question": "Q?", "options": []})), None);
    assert_eq!(AskUser::parse(&json!({"question": "Q?", "options": [{"id": "a"}]})), None);
    assert!(!AskUser::parse(&json!({"question": "Q?", "options": [{"id": "a", "label": "A"}], "allowFreeText": false})).unwrap().allow_free_text);
}

#[test]
fn one_choice_is_answered_with_its_label_or_the_typed_words() {
    let one = ask(false, true);
    assert_eq!(one.answer(&ids(&["b"]), ""), "Shields");
    assert_eq!(one.answer(&ids(&[]), "  Both, really  "), "Both, really");
    assert_eq!(one.answer(&ids(&[]), " "), "");
    // Without free text, typed words are not sent.
    assert_eq!(ask(false, false).answer(&ids(&[]), "mine"), "");
}

#[test]
fn several_choices_are_answered_one_line_each_then_the_words() {
    let several = ask(true, true);
    // In the card's order, whatever order they were picked in.
    assert_eq!(several.answer(&ids(&["c", "a"]), ""), "- Weapons\n- Economy");
    assert_eq!(several.answer(&ids(&["b"]), " and gold "), "- Shields\nand gold");
    assert_eq!(several.answer(&ids(&[]), "none of these"), "none of these");
}

#[test]
fn an_answer_is_read_back_into_the_card() {
    let one = ask(false, true);
    assert_eq!(one.read("Shields"), Answered { picked: ids(&["b"]), rest: String::new() });
    assert_eq!(one.read(" Shields \n"), Answered { picked: ids(&["b"]), rest: String::new() });
    assert_eq!(one.read("Something else"), Answered { picked: ids(&[]), rest: "Something else".into() });

    let several = ask(true, true);
    assert_eq!(several.read("- Weapons\n- Economy\nand gold"), Answered { picked: ids(&["a", "c"]), rest: "and gold".into() });
    // Typed by hand in the box, without the `- `.
    assert_eq!(several.read("Shields\n-Weapons"), Answered { picked: ids(&["a", "b"]), rest: String::new() });
}

#[test]
fn what_is_sent_reads_back_as_what_was_picked() {
    let several = ask(true, true);
    let sent = several.answer(&ids(&["a", "b"]), "mostly a");
    assert_eq!(several.read(&sent), Answered { picked: ids(&["a", "b"]), rest: "mostly a".into() });
}

#[test]
fn the_draft_toggles_or_replaces() {
    let one = ask(false, true);
    let mut draft = Draft::default();
    draft.typed(&one, "my own".into());
    draft.pick(&one, "a");
    assert_eq!((draft.picked.clone(), draft.typed.as_str()), (ids(&["a"]), ""));
    draft.pick(&one, "b");
    assert_eq!(draft.picked, ids(&["b"]));
    draft.typed(&one, "words".into());
    assert!(draft.picked.is_empty());

    let several = ask(true, true);
    let mut draft = Draft::default();
    draft.pick(&several, "a");
    draft.pick(&several, "c");
    draft.typed(&several, "and".into());
    draft.pick(&several, "a");
    assert_eq!((draft.picked.clone(), draft.typed.as_str()), (ids(&["c"]), "and"));
}
