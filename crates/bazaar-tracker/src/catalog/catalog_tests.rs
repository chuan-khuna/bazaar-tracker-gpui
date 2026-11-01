use std::time::Duration;

use super::*;

const PISTOL_ID: &str = "638a6b50-7803-4da5-9514-ca0627fd7b5f";

fn fixture_cards_json() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog/cards.json")).unwrap()
}

fn card_json(id: &str, title: &str, tier: &str) -> String {
    format!(r#"{{"Id":"{id}","InternalName":"{title}-internal","StartingTier":"{tier}","Size":"Small","Type":"Item","Localization":{{"Title":{{"Text":"{title}"}},"Description":{{"Text":"Does {title}."}}}}}}"#)
}

/// A `GameData.db` with a `cards` table holding these `(Id, JSON)` rows, the JSON as a blob.
fn write_game_data_db(path: &Path, cards: &[(&str, String)]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = Connection::open(path).unwrap();
    connection.execute("CREATE TABLE cards (Id TEXT PRIMARY KEY, Data BLOB)", []).unwrap();
    for (id, json) in cards {
        connection.execute("INSERT INTO cards (Id, Data) VALUES (?1, ?2)", rusqlite::params![id, json.as_bytes()]).unwrap();
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn age(path: &Path, seconds_ago: u64) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(seconds_ago)).unwrap();
}

#[test]
fn cards_json_gives_name_tier_size_type_and_description() {
    let catalog = CardCatalog::from_cards_json(&fixture_cards_json()).unwrap();
    assert_eq!(catalog.len(), 3);
    let pistol = catalog.get(PISTOL_ID).unwrap();
    assert_eq!(pistol.name, "Pistol of Testing");
    assert_eq!((pistol.starting_tier.as_deref(), pistol.size.as_deref(), pistol.card_type.as_deref()), (Some("Silver"), Some("Medium"), Some("Item")));
    assert_eq!(pistol.description.as_deref(), Some("Deal 30 damage."));
}

#[test]
fn cards_json_gives_the_heroes_that_can_get_a_card() {
    let catalog = CardCatalog::from_cards_json(&fixture_cards_json()).unwrap();
    assert_eq!(catalog.get(PISTOL_ID).unwrap().heroes, ["Vanessa", "Dooley"]);
    assert_eq!(catalog.get("da766a09-0352-4966-829f-20bda8da48d7").unwrap().heroes, [NEUTRAL_HERO]);
    assert!(catalog.get("skill-without-names").unwrap().heroes.is_empty());
}

#[test]
fn the_name_falls_back_from_the_title_to_the_internal_name_to_the_id() {
    let catalog = CardCatalog::from_cards_json(&fixture_cards_json()).unwrap();
    assert_eq!(catalog.get("da766a09-0352-4966-829f-20bda8da48d7").unwrap().name, "Plain Rock");
    assert_eq!(catalog.get("da766a09-0352-4966-829f-20bda8da48d7").unwrap().description, None);
    assert_eq!(catalog.get("skill-without-names").unwrap().name, "skill-without-names");
}

#[test]
fn lookup_by_id_ignores_case() {
    let catalog = CardCatalog::from_cards_json(&fixture_cards_json()).unwrap();
    // The file has the id in capitals; the game's log has it in lower case.
    assert!(catalog.get(PISTOL_ID).is_some());
    assert!(catalog.get(&PISTOL_ID.to_uppercase()).is_some());
    assert!(catalog.get("no-such-card").is_none());
}

#[test]
fn game_data_db_is_read_from_a_copy_and_the_copy_is_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("prod").join("cache").join("GameData.db");
    write_game_data_db(&db, &[(PISTOL_ID, card_json(PISTOL_ID, "Pistol", "Silver")), ("other", card_json("other", "Other", "Gold"))]);
    let catalog = CardCatalog::load(dir.path());
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog.get(&PISTOL_ID.to_uppercase()).unwrap().name, "Pistol");
    assert_eq!(catalog.source(), Some(db.as_path()));
    // Other tests in this process load catalogs at the same moment and have copies of their own,
    // so a copy seen right now may be theirs: it must be gone a moment later.
    // A file Windows has deleted but a scanner still holds open is listed but cannot be opened: it is gone.
    let leftovers = || -> Vec<_> {
        fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&format!("bazaar_tracker_gamedata_{}_", std::process::id())) && fs::File::open(entry.path()).is_ok())
            .collect()
    };
    let started = std::time::Instant::now();
    while !leftovers().is_empty() && started.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let left = leftovers();
    assert!(left.is_empty(), "the temporary copy was left behind: {left:?}");
}

#[test]
fn a_game_data_db_the_game_has_open_can_still_be_read() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("cache").join("GameData.db");
    write_game_data_db(&db, &[("a", card_json("a", "Alpha", "Bronze"))]);
    // The game holds the file open for writing while the catalog loads.
    let _game = fs::OpenOptions::new().read(true).write(true).open(&db).unwrap();
    assert_eq!(CardCatalog::load(dir.path()).get("a").unwrap().name, "Alpha");
}

#[test]
fn the_newer_file_wins_among_the_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let older = dir.path().join("prod").join("cache").join("GameData.db");
    let newer = dir.path().join("cache").join("GameData.db");
    write_game_data_db(&older, &[("c", card_json("c", "Old Name", "Bronze"))]);
    write_game_data_db(&newer, &[("c", card_json("c", "New Name", "Bronze"))]);
    age(&older, 3600);
    assert_eq!(CardCatalog::load(dir.path()).get("c").unwrap().name, "New Name");
    // And the other way round.
    age(&older, 0);
    age(&newer, 7200);
    assert_eq!(CardCatalog::load(dir.path()).get("c").unwrap().name, "Old Name");
}

#[test]
fn all_four_places_are_candidates() {
    let root = Path::new("root");
    let all: Vec<_> = candidates(root, "cards.json").into_iter().map(|path| path.to_string_lossy().replace('\\', "/")).collect();
    assert_eq!(all, ["root/prod/cache/cards.json", "root/cache/cards.json", "root/prod/cards.json", "root/cards.json"]);
    let dir = tempfile::tempdir().unwrap();
    write_game_data_db(&dir.path().join("prod").join("GameData.db"), &[("p", card_json("p", "In Prod", "Gold"))]);
    assert_eq!(CardCatalog::load(dir.path()).get("p").unwrap().name, "In Prod");
}

#[test]
fn a_broken_game_data_db_falls_back_to_cards_json() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("cache").join("GameData.db"), "this is not a database");
    write(&dir.path().join("cache").join("cards.json"), &fixture_cards_json());
    let catalog = CardCatalog::load(dir.path());
    assert_eq!(catalog.get(PISTOL_ID).unwrap().name, "Pistol of Testing");
    assert!(catalog.source().unwrap().ends_with("cards.json"));
}

#[test]
fn a_game_data_db_without_a_cards_table_falls_back_too() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("GameData.db");
    Connection::open(&db).unwrap().execute("CREATE TABLE other (x INTEGER)", []).unwrap();
    write(&dir.path().join("cards.json"), &fixture_cards_json());
    assert_eq!(CardCatalog::load(dir.path()).len(), 3);
}

#[test]
fn a_game_data_db_is_preferred_over_cards_json_even_if_cards_json_is_newer() {
    let dir = tempfile::tempdir().unwrap();
    write_game_data_db(&dir.path().join("cache").join("GameData.db"), &[("c", card_json("c", "From Db", "Bronze"))]);
    write(&dir.path().join("cache").join("cards.json"), &fixture_cards_json());
    age(&dir.path().join("cache").join("GameData.db"), 3600);
    assert_eq!(CardCatalog::load(dir.path()).get("c").unwrap().name, "From Db");
}

#[test]
fn nothing_found_gives_an_empty_catalog_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = CardCatalog::load(dir.path());
    assert!(catalog.is_empty());
    assert!(catalog.source().is_none());
    assert!(catalog.get("anything").is_none());
    let missing = CardCatalog::load(&dir.path().join("does-not-exist"));
    assert!(missing.is_empty());
}

#[test]
fn a_broken_cards_json_alone_gives_an_empty_catalog() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("cards.json"), "{ not json");
    assert!(CardCatalog::load(dir.path()).is_empty());
}

#[test]
fn cards_with_odd_json_are_skipped_not_fatal() {
    let text = r#"{"A":[{"Id":"ok","InternalName":"Fine"},{"NoId":true},{"Id":null},"nonsense"],"B":"not an array"}"#;
    let catalog = CardCatalog::from_cards_json(text).unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog.get("OK").unwrap().name, "Fine");
}

#[test]
fn loading_runs_off_the_callers_thread_and_once() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("cards.json"), &fixture_cards_json());
    let handle = CatalogHandle::start(dir.path().to_path_buf());
    let catalog = handle.wait(Duration::from_secs(10)).expect("loaded");
    assert_eq!(catalog.len(), 3);
    // Asking again returns the one catalog that was loaded.
    assert!(Arc::ptr_eq(&catalog, &handle.get().unwrap()));
    assert!(Arc::ptr_eq(&catalog, &handle.clone().wait(Duration::ZERO).unwrap()));
}

#[test]
fn the_handle_has_nothing_to_give_until_loading_is_done() {
    let dir = tempfile::tempdir().unwrap();
    // Nothing to load: the thread finishes with an empty catalog, which is still an answer.
    let handle = CatalogHandle::start(dir.path().join("nothing-here"));
    assert!(handle.wait(Duration::from_secs(10)).unwrap().is_empty());
}

/// Reads the real game data on this machine, if there is any. Run by hand with `--ignored`.
#[test]
#[ignore = "needs The Bazaar's data folder"]
fn the_real_game_data_loads() {
    let Some(profile) = crate::log::path::user_profile() else { return };
    let root = crate::log::path::game_folder_in(&profile);
    let catalog = CardCatalog::load(&root);
    println!("{} cards from {:?}", catalog.len(), catalog.source());
    for card in catalog.cards.values().take(3) {
        println!("{card:?}");
    }
    assert!(!catalog.is_empty());
}
