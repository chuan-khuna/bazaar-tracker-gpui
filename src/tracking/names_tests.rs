use super::*;

const CARDS: &str = r#"{ "Items": [ { "Id": "TPL-PISTOL", "InternalName": "Pistol", "Localization": { "Title": { "Text": "Pistol of Testing" } } } ] }"#;

fn loaded() -> Names {
    Names::new(Arc::new(CardCatalog::from_cards_json(CARDS).unwrap()))
}

#[test]
fn a_known_card_is_named_by_the_catalog_ignoring_case() {
    assert_eq!(loaded().card(Some("tpl-pistol"), "itm_1"), "Pistol of Testing");
}

#[test]
fn an_unknown_card_is_named_by_its_template_id() {
    assert_eq!(loaded().card(Some("TPL-OTHER"), "itm_1"), "TPL-OTHER");
}

#[test]
fn before_the_catalog_loads_cards_keep_their_template_id() {
    assert_eq!(Names::default().card(Some("TPL-PISTOL"), "itm_1"), "TPL-PISTOL");
}

#[test]
fn a_card_with_no_template_is_named_by_its_instance_id() {
    assert_eq!(loaded().card(None, "itm_1"), "itm_1");
}
