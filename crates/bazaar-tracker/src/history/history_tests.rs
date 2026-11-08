use std::sync::Arc;

use super::*;
use crate::memory::{PlayerStats, Snapshot};

fn database() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.db");
    (dir, path)
}

fn open() -> (tempfile::TempDir, History) {
    let (dir, path) = database();
    let history = History::open(&path).unwrap();
    (dir, history)
}

fn event(id: &str, run: &str, at: i64, kind: EventKind) -> Event {
    Event { id: id.to_owned(), run_id: run.to_owned(), timestamp_ms: at, day: None, hour: None, kind }
}

fn run_started(id: &str, run: &str, at: i64, hero: Option<&str>) -> Event {
    event(id, run, at, EventKind::RunStarted { play_mode: Some("Ranked".into()), build_id: None, environment: None, hero: hero.map(str::to_owned), rating: Some(1000), rank: None, rank_division: None })
}

fn rerolled(id: &str, run: &str, at: i64) -> Event {
    event(id, run, at, EventKind::Rerolled)
}

fn snapshot_with_hero(hero: &str) -> Snapshot {
    Snapshot { stats: PlayerStats { hero: Some(hero.to_owned()), ..PlayerStats::default() }, ..Snapshot::default() }
}

#[test]
fn a_fresh_file_gets_the_current_schema_and_foreign_keys() {
    let (_dir, history) = open();
    assert_eq!(history.schema_version().unwrap(), CURRENT_VERSION);
    let (tables, foreign_keys): (Vec<String>, bool) = history
        .with_writer(|connection| {
            let mut statement = connection.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
            let tables = statement.query_map([], |row| row.get(0))?.collect::<std::result::Result<Vec<String>, _>>()?;
            let on: bool = connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
            Ok((tables, on))
        })
        .unwrap();
    assert_eq!(tables, ["imported_item_odds", "imported_shop_odds", "run_tracker_events", "run_tracker_hidden_items", "run_tracker_runs", "run_tracker_tracked_items", "setting_pinned_items"].map(str::to_owned), "{tables:?}");
    assert!(foreign_keys);
}

#[test]
fn reopening_a_file_keeps_its_data() {
    let (_dir, path) = database();
    {
        let history = History::open(&path).unwrap();
        history.insert_event(&run_started("e1", "r1", 100, Some("Jules"))).unwrap();
    }
    let history = History::open(&path).unwrap();
    assert_eq!(history.list_runs().unwrap().len(), 1);
    assert_eq!(history.schema_version().unwrap(), CURRENT_VERSION);
}

#[test]
fn an_inserted_event_lists_and_loads_back_equal() {
    let (_dir, history) = open();
    let mut original = event(
        "e2",
        "r1",
        200,
        EventKind::Selected {
            card_kind: "Item".into(),
            instance_id: "itm_a".into(),
            template_id: Some("tpl".into()),
            tier: Some("Gold".into()),
            enchantment: None,
            section: Some("Player".into()),
            target: Some("PlayerSocket_3".into()),
            merged_into: None,
            options_event_id: Some("e1".into()),
            snapshot: snapshot_with_hero("Jules"),
        },
    );
    original.day = Some(4);
    original.hour = Some(2);
    assert!(history.insert_event(&run_started("e1", "r1", 100, None)).unwrap());
    assert!(history.insert_event(&original).unwrap());
    let events = history.list_events("r1").unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1], original);
    assert_eq!(history.load_event("e2").unwrap(), Some(original));
    assert_eq!(history.load_event("nope").unwrap(), None);
}

#[test]
fn saving_the_same_event_twice_changes_nothing() {
    let (_dir, history) = open();
    let e = rerolled("e1", "r1", 100);
    assert!(history.insert_event(&e).unwrap());
    assert!(!history.insert_event(&e).unwrap());
    assert_eq!(history.list_events("r1").unwrap().len(), 1);
}

#[test]
fn events_come_back_in_time_order_and_ties_in_insertion_order() {
    let (_dir, history) = open();
    history.insert_event(&rerolled("late", "r1", 300)).unwrap();
    history.insert_event(&rerolled("early", "r1", 100)).unwrap();
    history.insert_event(&rerolled("tie-a", "r1", 200)).unwrap();
    history.insert_event(&rerolled("tie-b", "r1", 200)).unwrap();
    let ids: Vec<_> = history.list_events("r1").unwrap().into_iter().map(|event| event.id).collect();
    assert_eq!(ids, ["early", "tie-a", "tie-b", "late"]);
}

#[test]
fn runs_list_newest_first() {
    let (_dir, history) = open();
    history.insert_event(&run_started("a", "old", 100, None)).unwrap();
    history.insert_event(&run_started("b", "new", 900, None)).unwrap();
    history.insert_event(&run_started("c", "mid", 500, None)).unwrap();
    let ids: Vec<_> = history.list_runs().unwrap().into_iter().map(|run| run.id).collect();
    assert_eq!(ids, ["new", "mid", "old"]);
}

#[test]
fn a_run_summary_follows_its_events() {
    let (_dir, history) = open();
    let mut started = run_started("e1", "r1", 1000, Some("Jules"));
    started.day = Some(1);
    history.insert_event(&started).unwrap();
    let run = history.get_run("r1").unwrap().unwrap();
    assert_eq!((run.started_ms, run.hero.as_deref(), run.day, run.ended, run.ended_ms), (1000, Some("Jules"), Some(1), false, None));

    let mut combat_end = event(
        "e2",
        "r1",
        2000,
        EventKind::CombatEnd {
            outcome: Outcome::Win,
            combat_start_event_id: None,
            player_hp_after: None,
            opponent_hp_after: None,
            victories_after: Some(3),
            losses_after: Some(1),
            sim_winner: None,
            sim_loser: None,
            sim_frames: None,
            expected_sim_frames: None,
            sim_diag: None,
        },
    );
    combat_end.day = Some(4);
    history.insert_event(&combat_end).unwrap();
    let run = history.get_run("r1").unwrap().unwrap();
    assert_eq!((run.victories, run.losses, run.day), (Some(3), Some(1), Some(4)));

    // An earlier day does not move the day back.
    let mut stale = rerolled("e3", "r1", 2500);
    stale.day = Some(2);
    history.insert_event(&stale).unwrap();
    assert_eq!(history.get_run("r1").unwrap().unwrap().day, Some(4));

    let mut ended = event(
        "e4",
        "r1",
        3000,
        EventKind::RunEnded { outcome: Outcome::Loss, victories: Some(3), losses: Some(3), gold: None, health_max: None, prestige: None, level: None, income: None, rating_after: None, rank_after: None, rank_division_after: None },
    );
    ended.day = Some(7);
    history.insert_event(&ended).unwrap();
    let run = history.get_run("r1").unwrap().unwrap();
    assert!(run.ended);
    assert_eq!((run.ended_ms, run.outcome, run.victories, run.losses, run.day), (Some(3000), Some(Outcome::Loss), Some(3), Some(3), Some(7)));
}

#[test]
fn the_hero_comes_from_a_snapshot_when_the_run_start_did_not_know_it() {
    let (_dir, history) = open();
    history.insert_event(&run_started("e1", "r1", 100, None)).unwrap();
    assert_eq!(history.get_run("r1").unwrap().unwrap().hero, None);
    history.insert_event(&event("e2", "r1", 200, EventKind::Sold { instance_id: "x".into(), template_id: None, tier: None, enchantment: None, snapshot: snapshot_with_hero("Vanessa") })).unwrap();
    assert_eq!(history.get_run("r1").unwrap().unwrap().hero.as_deref(), Some("Vanessa"));
}

#[test]
fn an_event_for_an_unknown_run_creates_it() {
    let (_dir, history) = open();
    history.insert_event(&rerolled("e1", "local-1", 700)).unwrap();
    let run = history.get_run("local-1").unwrap().unwrap();
    assert_eq!((run.started_ms, run.ended), (700, false));
}

#[test]
fn a_run_started_again_is_no_longer_ended() {
    let (_dir, history) = open();
    history.insert_event(&run_started("e1", "r1", 100, None)).unwrap();
    history.insert_event(&event("e2", "r1", 200, EventKind::RunEnded { outcome: Outcome::Win, victories: None, losses: None, gold: None, health_max: None, prestige: None, level: None, income: None, rating_after: None, rank_after: None, rank_division_after: None })).unwrap();
    assert!(history.get_run("r1").unwrap().unwrap().ended);
    history.insert_event(&run_started("e3", "r1", 300, None)).unwrap();
    let run = history.get_run("r1").unwrap().unwrap();
    assert!(!run.ended && run.ended_ms.is_none() && run.outcome.is_none());
}

#[test]
fn a_run_can_be_marked_ended_or_not() {
    let (_dir, history) = open();
    history.insert_event(&run_started("e1", "r1", 100, None)).unwrap();
    assert!(history.set_run_ended("r1", true).unwrap());
    assert!(history.get_run("r1").unwrap().unwrap().ended);
    assert!(history.set_run_ended("r1", false).unwrap());
    assert!(!history.get_run("r1").unwrap().unwrap().ended);
    assert!(!history.set_run_ended("missing", true).unwrap());
}

#[test]
fn deleting_a_run_removes_its_events_and_only_its_events() {
    let (_dir, history) = open();
    history.insert_event(&run_started("e1", "r1", 100, None)).unwrap();
    history.insert_event(&rerolled("e2", "r1", 200)).unwrap();
    history.insert_event(&run_started("e3", "r2", 300, None)).unwrap();
    assert!(history.delete_run("r1").unwrap());
    assert!(!history.delete_run("r1").unwrap());
    assert!(history.get_run("r1").unwrap().is_none());
    assert!(history.list_events("r1").unwrap().is_empty());
    assert!(history.load_event("e2").unwrap().is_none());
    assert_eq!(history.list_events("r2").unwrap().len(), 1);
}

fn foreign_key_violations(history: &History) -> usize {
    history
        .with_writer(|connection| {
            let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
            let count = statement.query_map([], |_| Ok(()))?.count();
            Ok(count)
        })
        .unwrap()
}

#[test]
fn rebinding_moves_the_run_and_its_events_under_foreign_keys() {
    let (_dir, history) = open();
    history.insert_event(&run_started("e1", "local-abc", 100, Some("Jules"))).unwrap();
    history.insert_event(&rerolled("e2", "local-abc", 200)).unwrap();
    assert!(history.rebind_run("local-abc", "real-1").unwrap());
    assert!(history.get_run("local-abc").unwrap().is_none());
    let run = history.get_run("real-1").unwrap().unwrap();
    assert_eq!((run.started_ms, run.hero.as_deref()), (100, Some("Jules")));
    let events = history.list_events("real-1").unwrap();
    assert_eq!(events.iter().map(|event| (event.id.as_str(), event.run_id.as_str())).collect::<Vec<_>>(), [("e1", "real-1"), ("e2", "real-1")]);
    assert!(history.list_events("local-abc").unwrap().is_empty());
    assert_eq!(foreign_key_violations(&history), 0);
}

#[test]
fn rebinding_into_a_run_that_exists_merges_them() {
    let (_dir, history) = open();
    history.insert_event(&rerolled("e1", "local-abc", 100)).unwrap();
    history.insert_event(&run_started("e2", "real-1", 300, Some("Jules"))).unwrap();
    history.insert_event(&rerolled("e3", "local-abc", 400)).unwrap();
    assert!(history.rebind_run("local-abc", "real-1").unwrap());
    assert_eq!(history.list_runs().unwrap().len(), 1);
    let run = history.get_run("real-1").unwrap().unwrap();
    assert_eq!(run.started_ms, 100, "the merged run keeps the earlier start");
    let ids: Vec<_> = history.list_events("real-1").unwrap().into_iter().map(|event| event.id).collect();
    assert_eq!(ids, ["e1", "e2", "e3"]);
    assert_eq!(foreign_key_violations(&history), 0);
}

#[test]
fn rebinding_a_run_that_is_not_there_does_nothing() {
    let (_dir, history) = open();
    history.insert_event(&rerolled("e1", "real-1", 100)).unwrap();
    assert!(!history.rebind_run("local-none", "real-1").unwrap());
    assert!(!history.rebind_run("real-1", "real-1").unwrap());
    assert_eq!(history.list_events("real-1").unwrap().len(), 1);
}

#[test]
fn each_migration_runs_once_and_only_on_files_that_lack_it() {
    let (_dir, path) = database();
    let first = "CREATE TABLE a (n INTEGER); INSERT INTO a VALUES (1);";
    let second = "INSERT INTO a VALUES (2);";
    let count = |history: &History| -> i64 { history.with_reader(|c| Ok(c.query_row("SELECT COUNT(*) FROM a", [], |row| row.get(0))?)).unwrap() };

    let history = History::open_with_migrations(&path, &[first]).unwrap();
    assert_eq!((history.schema_version().unwrap(), count(&history)), (1, 1));
    drop(history);

    // Opening again with the same migrations runs nothing more.
    let history = History::open_with_migrations(&path, &[first]).unwrap();
    assert_eq!((history.schema_version().unwrap(), count(&history)), (1, 1));
    drop(history);

    // A newer app adds a migration: only the new one runs, once.
    let history = History::open_with_migrations(&path, &[first, second]).unwrap();
    assert_eq!((history.schema_version().unwrap(), count(&history)), (2, 2));
    drop(history);
    let history = History::open_with_migrations(&path, &[first, second]).unwrap();
    assert_eq!((history.schema_version().unwrap(), count(&history)), (2, 2));
}

#[test]
fn a_failing_migration_leaves_the_file_as_it_was() {
    let (_dir, path) = database();
    let first = "CREATE TABLE a (n INTEGER);";
    drop(History::open_with_migrations(&path, &[first]).unwrap());
    let broken = "INSERT INTO a VALUES (1); INSERT INTO missing VALUES (1);";
    assert!(History::open_with_migrations(&path, &[first, broken]).is_err());
    let history = History::open_with_migrations(&path, &[first]).unwrap();
    assert_eq!(history.schema_version().unwrap(), 1);
    assert_eq!(history.with_reader(|c| Ok(c.query_row("SELECT COUNT(*) FROM a", [], |row| row.get::<_, i64>(0))?)).unwrap(), 0);
}

#[test]
fn a_file_from_a_newer_app_is_refused() {
    let (_dir, path) = database();
    drop(History::open(&path).unwrap());
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", CURRENT_VERSION + 5).unwrap();
    drop(connection);
    let error = History::open(&path).err().unwrap();
    assert!(matches!(error, Error::NewerSchema { found, supported } if found == CURRENT_VERSION + 5 && supported == CURRENT_VERSION), "{error}");
}

#[test]
fn reads_run_alongside_writes() {
    let (_dir, history) = open();
    let history = Arc::new(history);
    history.insert_event(&run_started("start", "r1", 1, None)).unwrap();
    let writer = {
        let history = history.clone();
        std::thread::spawn(move || {
            for i in 0..300 {
                history.insert_event(&rerolled(&format!("w{i}"), "r1", 10 + i)).unwrap();
            }
        })
    };
    let reader = {
        let history = history.clone();
        std::thread::spawn(move || {
            let mut last = 0;
            for _ in 0..100 {
                let count = history.list_events("r1").unwrap().len();
                assert!(count >= last, "events never disappear");
                last = count;
                history.list_runs().unwrap();
            }
        })
    };
    writer.join().unwrap();
    reader.join().unwrap();
    assert_eq!(history.list_events("r1").unwrap().len(), 301);
}

#[test]
fn writes_from_several_threads_are_all_saved() {
    let (_dir, history) = open();
    let history = Arc::new(history);
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let history = history.clone();
            std::thread::spawn(move || {
                for i in 0..50 {
                    history.insert_event(&rerolled(&format!("t{t}-{i}"), &format!("r{t}"), i)).unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(history.list_runs().unwrap().len(), 4);
    for t in 0..4 {
        assert_eq!(history.list_events(&format!("r{t}")).unwrap().len(), 50);
    }
}

#[test]
fn clearing_deletes_runs_events_items_and_odds_but_keeps_the_schema() {
    let (_dir, history) = open();
    history.insert_event(&run_started("e1", "r1", 1, Some("Vanessa"))).unwrap();
    history.track_item("r1", "card-a", 2).unwrap();
    history.hide_item("r1", "card-b", 3).unwrap();
    history.seed_shop_odds(&bazaardb::shop_odds::defaults(), 4).unwrap();
    history
        .save_item_odds(&SavedItemOdds { template_id: "t".into(), hero: "Jules".into(), tier: "Bronze".into(), url_id: "u".into(), item_name: "Pan".into(), body: "{}".into(), saved_ms: 5 })
        .unwrap();

    history.clear().unwrap();

    assert!(history.list_runs().unwrap().is_empty());
    assert!(history.load_event("e1").unwrap().is_none());
    assert!(history.tracked_items("r1").unwrap().is_empty());
    assert!(history.hidden_items("r1").unwrap().is_empty());
    assert!(history.shop_odds().unwrap().odds.days.is_empty());
    assert!(history.item_odds().unwrap().is_empty());
    assert_eq!(history.schema_version().unwrap(), CURRENT_VERSION);
    // It still takes new Runs.
    assert!(history.insert_event(&run_started("e2", "r2", 6, None)).unwrap());
}

/// Make a file at schema 7 with a row in every table, as the app before migration 8 left it.
fn file_at_version_7(path: &std::path::Path) {
    drop(History::open_with_migrations(path, &MIGRATIONS[..7]).unwrap());
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            r#"
            INSERT INTO runs (id, started_ms, hero, day) VALUES ('r1', 100, 'Jules', 3);
            INSERT INTO events (id, run_id, kind, timestamp_ms, details, upload_status) VALUES ('e1', 'r1', 'Rerolled', 100, '{"type":"Rerolled"}', 1);
            INSERT INTO tracked_items VALUES ('r1', 'tpl-a', 1);
            INSERT INTO hidden_items VALUES ('r1', 'tpl-b', 2);
            INSERT INTO pinned_items VALUES ('Jules', 'tpl-c', 3);
            INSERT INTO shop_odds VALUES (1, 10, 0.7, 0.2, 0.1, 0.0, 40);
            INSERT INTO item_odds VALUES ('tpl-a', 'Jules', 'None', 'u', 'Pan', '{}', 50);
            "#,
        )
        .unwrap();
}

#[test]
fn migration_8_keeps_every_row_under_the_prefixed_names() {
    let (_dir, path) = database();
    file_at_version_7(&path);
    let history = History::open(&path).unwrap();
    assert_eq!(history.schema_version().unwrap(), 8);

    assert_eq!(history.get_run("r1").unwrap().unwrap().hero.as_deref(), Some("Jules"));
    assert_eq!(history.list_events("r1").unwrap().len(), 1);
    assert_eq!(history.upload_status("e1").unwrap(), Some(UploadStatus::Sent));
    assert_eq!(history.tracked_items("r1").unwrap(), ["tpl-a"]);
    assert_eq!(history.hidden_items("r1").unwrap(), ["tpl-b"]);
    assert_eq!(history.pinned_items("Jules").unwrap(), ["tpl-c"]);
    assert_eq!(history.shop_odds().unwrap().saved_ms, Some(40));
    assert_eq!(history.item_odds().unwrap()[0].saved_ms, 50);

    let (sources, indexes, broken): (Vec<String>, Vec<String>, i64) = history
        .with_reader(|c| {
            let sources = c.prepare("SELECT source FROM imported_shop_odds UNION ALL SELECT source FROM imported_item_odds")?.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
            let indexes = c.prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL ORDER BY name")?.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
            let broken = c.prepare("PRAGMA foreign_key_check")?.query_map([], |_| Ok(()))?.count() as i64;
            Ok((sources, indexes, broken))
        })
        .unwrap();
    assert_eq!(sources, ["bazaardb", "bazaardb"]);
    assert_eq!(indexes, ["run_tracker_events_by_run", "run_tracker_events_waiting_for_upload", "run_tracker_runs_by_start"]);
    assert_eq!(broken, 0);

    // The foreign keys follow the renamed tables: deleting the Run still cascades.
    assert!(history.delete_run("r1").unwrap());
    assert!(history.list_events("r1").unwrap().is_empty());
    assert!(history.tracked_items("r1").unwrap().is_empty());
    assert_eq!(history.pinned_items("Jules").unwrap(), ["tpl-c"], "pinned items belong to the hero");
}

#[test]
fn an_older_file_is_backed_up_as_it_was_before_it_is_migrated() {
    let (dir, path) = database();
    file_at_version_7(&path);
    drop(History::open(&path).unwrap());

    let backup = dir.path().join("history.v7.bak.db");
    let connection = Connection::open(&backup).unwrap();
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    let runs: i64 = connection.query_row("SELECT COUNT(*) FROM runs", [], |row| row.get(0)).unwrap();
    assert_eq!((version, runs), (7, 1));
    drop(connection);

    // Opening the migrated file again backs nothing up; a second older file takes the next name.
    drop(History::open(&path).unwrap());
    assert!(!dir.path().join("history.v7.bak.2.db").exists());
    std::fs::remove_file(&path).unwrap();
    file_at_version_7(&path);
    drop(History::open(&path).unwrap());
    assert!(dir.path().join("history.v7.bak.2.db").exists());
}

#[test]
fn a_new_file_is_not_backed_up() {
    let (dir, _history) = open();
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    assert!(names.iter().all(|name| !name.contains(".bak")), "{names:?}");
}

#[test]
fn seeded_shop_odds_say_they_are_built_in_and_fetched_ones_bazaar_db() {
    let (_dir, history) = open();
    let source = |history: &History| -> String { history.with_reader(|c| Ok(c.query_row("SELECT DISTINCT source FROM imported_shop_odds", [], |row| row.get(0))?)).unwrap() };
    history.seed_shop_odds(&bazaardb::shop_odds::defaults(), 1).unwrap();
    assert_eq!(source(&history), "builtin");
    history.save_shop_odds(&bazaardb::shop_odds::defaults(), 2).unwrap();
    assert_eq!(source(&history), "bazaardb");
}

#[test]
fn imported_item_odds_say_they_came_from_the_scraper() {
    let (_dir, history) = open();
    let odds = SavedItemOdds { template_id: "t".into(), hero: "Jules".into(), tier: "None".into(), url_id: "u".into(), item_name: "Pan".into(), body: "{}".into(), saved_ms: 5 };
    history.import_item_odds(std::slice::from_ref(&odds)).unwrap();
    let source: String = history.with_reader(|c| Ok(c.query_row("SELECT source FROM imported_item_odds", [], |row| row.get(0))?)).unwrap();
    assert_eq!(source, "scraper");
}
