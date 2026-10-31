use std::sync::mpsc;
use std::sync::atomic::AtomicBool;

use super::*;

#[test]
fn five_failed_board_reads_in_a_row_give_up() {
    let mut failures = BoardFailures::default();
    for _ in 0..4 {
        assert!(!failures.record(true));
    }
    assert!(failures.record(true), "the fifth failure in a row");
}

#[test]
fn a_good_board_read_starts_the_count_again() {
    let mut failures = BoardFailures::default();
    for _ in 0..4 {
        failures.record(true);
    }
    assert!(!failures.record(false));
    for _ in 0..4 {
        assert!(!failures.record(true));
    }
    assert!(failures.record(true));
}

#[test]
fn a_reset_forgets_failures() {
    let mut failures = BoardFailures::default();
    for _ in 0..4 {
        failures.record(true);
    }
    failures.reset();
    assert!(!failures.record(true));
}

#[test]
fn a_guid_struct_is_formatted_in_the_usual_text() {
    // 638a6b50-7803-4da5-9514-ca0627fd7b5f as .NET lays it out: three little-endian fields, then bytes.
    let bytes = [0x50, 0x6b, 0x8a, 0x63, 0x03, 0x78, 0xa5, 0x4d, 0x95, 0x14, 0xca, 0x06, 0x27, 0xfd, 0x7b, 0x5f];
    assert_eq!(guid_from_bytes(&bytes), "638a6b50-7803-4da5-9514-ca0627fd7b5f");
    assert_eq!(guid_from_bytes(&[0; 16]), "00000000-0000-0000-0000-000000000000");
}

#[test]
fn a_rank_record_is_plausible_only_within_the_originals_ranges() {
    assert!(plausible_rank(3, 2, 1, 1234));
    assert!(plausible_rank(10_000, 16, 10, 1_000_000));
    assert!(!plausible_rank(0, 2, 1, 1234), "season 0");
    assert!(!plausible_rank(10_001, 2, 1, 1234), "season too large");
    assert!(!plausible_rank(3, 17, 1, 1234), "rank too large");
    assert!(!plausible_rank(3, 2, 0, 1234), "division 0");
    assert!(!plausible_rank(3, 2, 11, 1234), "division too large");
    assert!(!plausible_rank(3, 2, 1, 1_000_001), "rating too large");
}

#[test]
fn cards_are_sorted_into_the_games_order_and_unknown_ones_go_last() {
    let item = |id: &str| Item { instance_id: id.to_owned(), ..Item::default() };
    let mut items = vec![item("skl_c"), item("skl_x"), item("skl_a"), item("skl_b")];
    sort_by_order(&mut items, &["skl_a".to_owned(), "skl_b".to_owned(), "skl_c".to_owned()]);
    let ids: Vec<_> = items.iter().map(|item| item.instance_id.as_str()).collect();
    assert_eq!(ids, ["skl_a", "skl_b", "skl_c", "skl_x"]);
}

#[test]
fn not_initialised_is_not_available_yet_and_other_errors_are_kept() {
    assert!(matches!(lenient::<i32>(Err(Error::NotInitialised("TheBazaar.Data".into()))), Ok(None)));
    assert!(matches!(lenient(Ok(Some(3))), Ok(Some(3))));
    assert!(matches!(lenient::<i32>(Err(Error::ProcessGone)), Err(Error::ProcessGone)));
}

#[test]
fn auto_property_storage_has_the_compiler_name() {
    assert_eq!(prop("IsStorageOpen"), "<IsStorageOpen>k__BackingField");
}

#[test]
fn with_no_game_running_it_stays_detached_and_reads_say_so() {
    let mut memory = BazaarMemory::for_process("no-such-game-5f3a");
    let (sender, receiver) = mpsc::channel();
    memory.report_changes_to(sender);
    memory.tick();
    memory.tick();
    assert!(!memory.is_connected());
    assert!(receiver.try_recv().is_err(), "nothing changed, so nothing is reported");
    assert_eq!(memory.read_board(), Err(MemoryError::Disconnected));
    assert_eq!(memory.read_run_init(), Err(MemoryError::Disconnected));
    assert_eq!(memory.read_player_rank(), Err(MemoryError::Disconnected));
}

#[test]
fn the_connection_loop_stops_when_asked() {
    let memory = BazaarMemory::for_process("no-such-game-5f3a");
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let (memory, stop) = (memory.clone(), stop.clone());
        std::thread::spawn(move || memory.run_connection_loop(&stop))
    };
    std::thread::sleep(Duration::from_millis(100));
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}
