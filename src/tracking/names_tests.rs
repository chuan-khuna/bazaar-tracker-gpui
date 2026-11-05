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

const WITH_ART: &str = r#"{ "Items": [
    { "Id": "TPL-SWORD", "Type": "Item", "Size": "Small", "ArtKey": "ABC123" },
    { "Id": "TPL-SKILL", "Type": "Skill", "ArtKey": "Icon_Skill_Fast.png" },
    { "Id": "TPL-BARE", "Type": "Item", "ArtKey": "missing" }
] }"#;

#[test]
fn art_is_found_by_type_and_art_key_when_its_file_is_there() {
    let dir = tempfile::tempdir().unwrap();
    for file in ["items/abc123.webp", "skills/icon_skill_fast.webp"] {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }
    let index = ArtIndex::load(dir.path());
    assert_eq!(index.len(), 2);
    let names = Names::new(Arc::new(CardCatalog::from_cards_json(WITH_ART).unwrap())).with_art(Arc::new(index));
    assert_eq!(names.art(Some("tpl-sword")), Some(dir.path().join("items/abc123.webp")));
    assert_eq!(names.art(Some("TPL-SKILL")), Some(dir.path().join("skills/icon_skill_fast.webp")));
    assert_eq!(names.art(Some("TPL-BARE")), None);
    assert_eq!(names.art(None), None);
    assert_eq!(Names::default().art(Some("TPL-SWORD")), None);
}

#[test]
fn encounters_have_no_art_even_when_their_file_is_there() {
    let dir = tempfile::tempdir().unwrap();
    for file in ["monsters/m1.webp", "events/e1.webp", "pedestals/p1.webp"] {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }
    let cards = r#"{ "Items": [
        { "Id": "TPL-MONSTER", "Type": "CombatEncounter", "ArtKey": "M1" },
        { "Id": "TPL-EVENT", "Type": "EventEncounter", "ArtKey": "E1" },
        { "Id": "TPL-PEDESTAL", "Type": "PedestalEncounter", "ArtKey": "P1" }
    ] }"#;
    let names = Names::new(Arc::new(CardCatalog::from_cards_json(cards).unwrap())).with_art(Arc::new(ArtIndex::load(dir.path())));
    for id in ["TPL-MONSTER", "TPL-EVENT", "TPL-PEDESTAL"] {
        assert_eq!(names.art(Some(id)), None, "{id}");
    }
}

#[test]
fn a_missing_art_folder_is_an_empty_index() {
    assert!(ArtIndex::load(Path::new("no/such/folder")).is_empty());
}

#[test]
fn the_items_are_the_item_cards_by_name() {
    let cards = r#"{ "Items": [
        { "Id": "TPL-B", "Type": "Item", "InternalName": "Bowl" },
        { "Id": "TPL-A", "Type": "Item", "InternalName": "Anvil" },
        { "Id": "TPL-S", "Type": "Skill", "InternalName": "Aim" } ] }"#;
    let names = Names::new(Arc::new(CardCatalog::from_cards_json(cards).unwrap()));
    assert_eq!(names.items(), [("tpl-a".to_owned(), "Anvil".to_owned()), ("tpl-b".to_owned(), "Bowl".to_owned())]);
    assert!(Names::default().items().is_empty());
}

const ITEMS: &str = r#"{ "Items": [
    { "Id": "TPL-PASTA", "Type": "Item", "Heroes": ["Jules"], "Localization": { "Title": { "Text": "Pasta Maker" } } },
    { "Id": "TPL-SKILL", "Type": "Skill", "Heroes": ["Jules"] }
] }"#;

#[test]
fn the_items_file_lists_each_item_with_its_lower_cased_id_name_and_heroes() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = CardCatalog::from_cards_json(ITEMS).unwrap();
    assert_eq!(write_items_file(&catalog, dir.path()).unwrap(), 1);
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(ITEMS_FILE)).unwrap()).unwrap();
    assert_eq!(written, serde_json::json!({ "format": 1, "items": [ { "template_id": "tpl-pasta", "name": "Pasta Maker", "heroes": ["Jules"] } ] }));
}

#[test]
fn a_catalog_without_items_leaves_the_items_file_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(ITEMS_FILE), "earlier").unwrap();
    assert_eq!(write_items_file(&CardCatalog::from_cards_json(CARDS).unwrap(), dir.path()).unwrap(), 0);
    assert_eq!(std::fs::read_to_string(dir.path().join(ITEMS_FILE)).unwrap(), "earlier");
}
