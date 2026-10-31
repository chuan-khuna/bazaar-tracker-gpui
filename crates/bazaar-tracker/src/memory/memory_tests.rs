use super::fake::FakeMemory;
use super::*;

fn board_with_gold(gold: i32) -> BoardState {
    BoardState { player: Snapshot { stats: PlayerStats { gold: Some(gold), ..PlayerStats::default() }, ..Snapshot::default() }, ..BoardState::default() }
}

fn gold(board: MemoryResult<BoardState>) -> Option<i32> {
    board.unwrap().and_then(|board| board.player.stats.gold)
}

#[test]
fn nothing_scripted_is_not_available_yet() {
    let mut memory = FakeMemory::new();
    assert_eq!(memory.read_board(), Ok(None));
    assert_eq!(memory.read_run_init(), Ok(None));
    assert_eq!(memory.read_combat_result(), Ok(None));
    assert_eq!(memory.read_combat_frames(), Ok(None));
    assert_eq!(memory.read_pvp_opponent(), Ok(None));
    assert_eq!(memory.read_selected_hero(), Ok(None));
    assert_eq!(memory.read_player_rank(), Ok(None));
}

#[test]
fn a_set_value_repeats() {
    let mut memory = FakeMemory::new();
    memory.set_selected_hero("Jules".to_owned());
    assert_eq!(memory.read_selected_hero(), Ok(Some("Jules".to_owned())));
    assert_eq!(memory.read_selected_hero(), Ok(Some("Jules".to_owned())));
    memory.set_selected_hero(None);
    assert_eq!(memory.read_selected_hero(), Ok(None));
}

#[test]
fn pushed_values_come_in_order_and_then_the_last_repeats() {
    let mut memory = FakeMemory::new();
    memory.push_board(board_with_gold(1));
    memory.push_board(board_with_gold(2));
    memory.push_board(None);
    memory.push_board(board_with_gold(4));
    let reads: Vec<_> = (0..6).map(|_| gold(memory.read_board())).collect();
    assert_eq!(reads, [Some(1), Some(2), None, Some(4), Some(4), Some(4)]);
}

#[test]
fn a_failing_read_is_an_error_and_can_be_cleared() {
    let mut memory = FakeMemory::new();
    memory.fail_board("boom");
    assert_eq!(memory.read_board(), Err(MemoryError::Failed("boom".to_owned())));
    memory.set_board(board_with_gold(7));
    assert_eq!(gold(memory.read_board()), Some(7));
}

#[test]
fn clones_share_their_scripts_and_calls_are_counted() {
    let mut memory = FakeMemory::new();
    let script = memory.clone();
    assert_eq!(memory.board_calls(), 0);
    memory.read_board().unwrap();
    script.set_board(board_with_gold(9));
    assert_eq!(gold(memory.read_board()), Some(9));
    assert_eq!(script.board_calls(), 2);
    assert_eq!(script.player_rank_calls(), 0);
}

#[test]
fn the_fake_can_be_used_as_a_trait_object_across_threads() {
    let memory: Box<dyn GameMemory> = Box::new(FakeMemory::new());
    std::thread::spawn(move || {
        let mut memory = memory;
        assert_eq!(memory.read_player_rank(), Ok(None));
    })
    .join()
    .unwrap();
}

#[test]
fn a_board_state_finds_entities_and_lists_the_players_items() {
    let item = |id: &str| Item { instance_id: id.to_owned(), ..Item::default() };
    let board = BoardState {
        player: Snapshot { board: vec![item("itm_a")], stash: vec![item("itm_b")], skills: vec![item("skl_c")], stats: PlayerStats::default() },
        entities: vec![item("itm_a"), item("enc_x")],
        ..BoardState::default()
    };
    assert_eq!(board.entity("enc_x").map(|item| item.instance_id.as_str()), Some("enc_x"));
    assert!(board.entity("itm_none").is_none());
    assert_eq!(board.player_item_ids(), ["itm_a", "itm_b", "skl_c"]);
}
