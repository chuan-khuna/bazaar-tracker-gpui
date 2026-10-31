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
fn a_missing_art_folder_is_an_empty_index() {
    assert!(ArtIndex::load(Path::new("no/such/folder")).is_empty());
}
