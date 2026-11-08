use super::*;

const BODY: &str = r#"{"itemName":"Pasta Maker","itemUrlId":"qnwx","hero":"Jules","days":["1"],"ownedDataByDay":[true],"merchants":[]}"#;

fn file(entries: &[(&str, &str, &str)]) -> String {
    let odds: Vec<serde_json::Value> = entries
        .iter()
        .map(|(hero, tier, body)| {
            serde_json::json!({ "template_id": "E45D-ABC", "hero": hero, "tier": tier, "url_id": "qnwx",
                                "item_name": "Pasta Maker", "body": body, "saved_ms": 100 })
        })
        .collect();
    serde_json::json!({ "format": 1, "template_id": "e45d-abc", "item_name": "Pasta Maker", "url_id": "qnwx", "odds": odds }).to_string()
}

#[test]
fn each_entry_of_a_file_becomes_saved_odds_with_the_id_lower_cased() {
    let mut read = Read::default();
    read_file("e45d-abc.json", &file(&[("Jules", "None", BODY), ("Jules", "Gold", BODY)]), &mut read);
    assert_eq!(read.files, 1);
    assert!(read.problems.is_empty());
    assert_eq!(read.odds.iter().map(|odds| (odds.template_id.as_str(), odds.tier.as_str())).collect::<Vec<_>>(), [("e45d-abc", "None"), ("e45d-abc", "Gold")]);
    assert_eq!(read.odds[0].body, BODY);
    assert_eq!(read.odds[0].saved_ms, 100);
}

#[test]
fn an_entry_whose_answer_can_not_be_read_is_left_out_and_named() {
    let mut read = Read::default();
    read_file("e45d-abc.json", &file(&[("Jules", "None", "not json"), ("Jules", "Gold", BODY)]), &mut read);
    assert_eq!(read.odds.len(), 1);
    assert_eq!(read.problems.len(), 1);
    assert!(read.problems[0].starts_with("e45d-abc.json: the odds for Pasta Maker (Jules, None)"), "{}", read.problems[0]);
}

#[test]
fn a_file_of_another_format_or_not_json_is_a_problem_not_odds() {
    let mut read = Read::default();
    read_file("a.json", r#"{"format":2,"odds":[]}"#, &mut read);
    read_file("b.json", "nope", &mut read);
    assert_eq!(read.files, 0);
    assert!(read.odds.is_empty());
    assert_eq!(read.problems.len(), 2);
}

#[test]
fn a_folder_is_read_without_its_index_and_skipped_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("e45d-abc.json"), file(&[("Jules", "None", BODY)])).unwrap();
    std::fs::write(dir.path().join("index.json"), r#"{"format":1,"items":[]}"#).unwrap();
    std::fs::write(dir.path().join("skipped.json"), r#"{"format":1,"skipped":[]}"#).unwrap();
    std::fs::write(dir.path().join("notes.txt"), "not odds").unwrap();
    let read = read(dir.path()).unwrap();
    assert_eq!(read.files, 1);
    assert_eq!(read.odds.len(), 1);
    assert!(read.problems.is_empty(), "{:?}", read.problems);
}

#[test]
fn one_file_can_be_read_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("e45d-abc.json");
    std::fs::write(&path, file(&[("Jules", "None", BODY)])).unwrap();
    assert_eq!(read(&path).unwrap().odds.len(), 1);
}
