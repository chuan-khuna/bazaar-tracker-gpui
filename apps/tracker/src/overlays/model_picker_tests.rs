use super::*;

fn option(id: &str, name: &str) -> ModelOption {
    ModelOption { id: id.into(), name: name.into() }
}

fn models() -> Vec<ModelOption> {
    vec![option("anthropic/claude-sonnet", "Anthropic: Claude Sonnet"), option("nvidia/nemotron:free", "NVIDIA: Nemotron (free)"), option("openai/gpt-mini", "OpenAI: GPT Mini")]
}

fn ids(rows: &[Row]) -> Vec<Option<&str>> {
    rows.iter().map(Row::model).collect()
}

#[test]
fn an_empty_query_lists_the_backends_default_then_every_model() {
    let rows = rows("", &models(), Some("nvidia/nemotron:free"));
    assert_eq!(rows[0], Row::Backend(Some("nvidia/nemotron:free".into())));
    assert_eq!(ids(&rows), [None, Some("anthropic/claude-sonnet"), Some("nvidia/nemotron:free"), Some("openai/gpt-mini")]);
}

#[test]
fn each_word_must_be_in_the_id_or_the_name() {
    assert_eq!(ids(&rows("FREE nvidia", &models(), None)), [Some("nvidia/nemotron:free")]);
    assert_eq!(ids(&rows("anthropic sonnet", &models(), None)), [Some("anthropic/claude-sonnet")]);
    // By the name only.
    assert_eq!(ids(&rows("Mini", &models(), None))[0], Some("openai/gpt-mini"));
}

#[test]
fn a_model_not_in_the_list_can_be_typed_in() {
    let rows = rows("local/llama3:8b", &models(), None);
    assert_eq!(rows, [Row::Typed("local/llama3:8b".into())]);
    // A query that is a listed id isn't offered twice; one with spaces is no id.
    assert_eq!(ids(&super::rows("openai/gpt-mini", &models(), None)), [Some("openai/gpt-mini")]);
    assert!(super::rows("no such", &models(), None).is_empty());
}

#[test]
fn enter_chooses_and_shift_enter_also_sets_the_default() {
    let mut picker = ModelPicker::new(false);
    picker.press("down", None, false, 3);
    assert_eq!(picker.press("enter", None, false, 3), Outcome::Choose { index: 1, default: false });
    assert_eq!(picker.press("enter", None, true, 3), Outcome::Choose { index: 1, default: true });
    // From Settings, choosing is setting the default.
    assert_eq!(ModelPicker::new(true).press("enter", None, false, 3), Outcome::Choose { index: 0, default: true });
    assert_eq!(picker.press("escape", None, false, 3), Outcome::Close);
    assert_eq!(ModelPicker::new(false).press("enter", None, false, 0), Outcome::None);
}

#[test]
fn typing_filters_from_the_top_and_up_down_wrap() {
    let mut picker = ModelPicker::new(false);
    picker.press("up", None, false, 4);
    assert_eq!(picker.selected, 3);
    picker.press("a", Some("a"), false, 4);
    assert_eq!((picker.query.as_str(), picker.selected), ("a", 0));
    picker.press("backspace", None, false, 4);
    assert_eq!(picker.query, "");
}

#[test]
fn the_visible_rows_follow_the_highlight() {
    assert_eq!(visible(0, 3), 0..3);
    assert_eq!(visible(0, 100), 0..VISIBLE_ROWS);
    assert_eq!(visible(VISIBLE_ROWS, 100), 1..VISIBLE_ROWS + 1);
    assert_eq!(visible(99, 100), 100 - VISIBLE_ROWS..100);
    assert_eq!(visible(0, 0), 0..0);
}
