use crate::memory::{BoardState, CombatResult, PlayerRank, PvpOpponent, RunInit, Snapshot};
use crate::run::test_support::*;
use crate::run::{EventKind, Outcome, Output};

const PREPARED: &str = "[CombatSimHandler] Combat prepared: frames=659, effectsMs=5";

/// A board with the player's `a` on the board and an opponent with `o`, with these HP, wins and losses.
fn fight_board(player_items: &[&str], opponent_items: &[&str], player_hp: i32, opponent_hp: i32, victories: i32, losses: i32) -> BoardState {
    let mut board = board(&player_items.iter().map(|id| item(id, "Bronze", "Small")).collect::<Vec<_>>(), &[], 10);
    board.player.stats.health = Some(player_hp);
    board.player.stats.victories = Some(victories);
    board.player.stats.losses = Some(losses);
    board.opponent = Snapshot { board: opponent_items.iter().map(|id| item(id, "Silver", "Medium")).collect(), ..Snapshot::default() };
    board.opponent.stats.health = Some(opponent_hp);
    board.opponent.stats.health_max = Some(opponent_hp.max(1));
    board.run_state = Some("Combat".into());
    board
}

fn sim(winner: i32, loser: i32, frames: i32) -> CombatResult {
    CombatResult { winner: Some(winner), loser: Some(loser), frame_count: Some(frames) }
}

/// outcome, start id, player HP, opponent HP, victories, losses
type Ended<'a> = (&'a Outcome, Option<&'a str>, Option<i32>, Option<i32>, Option<i32>, Option<i32>);

fn combat_end_of(event: &crate::run::Event) -> Ended<'_> {
    let EventKind::CombatEnd { outcome, combat_start_event_id, player_hp_after, opponent_hp_after, victories_after, losses_after, .. } = &event.kind else { panic!("{:?}", event.kind) };
    (outcome, combat_start_event_id.as_deref(), *player_hp_after, *opponent_hp_after, *victories_after, *losses_after)
}

#[test]
fn a_pve_combat_has_both_boards_from_just_before_the_fight_and_an_outcome() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();

    h.state("ChoiceState", "CombatState");
    h.advance(350);
    // The fight changes the boards, but the capture already has the "before" picture.
    h.memory.set_board(fight_board(&[], &[], 120, 0, 1, 0));
    h.raw(PREPARED);
    let events = h.events();
    assert_eq!(names(&events), ["CombatStart"]);
    let EventKind::CombatStart { player, opponent, is_pvp, opponent_name, .. } = &events[0].kind else { panic!() };
    assert_eq!(player.board.iter().map(|i| i.instance_id.as_str()).collect::<Vec<_>>(), ["itm_a"]);
    assert_eq!(opponent.board.iter().map(|i| i.instance_id.as_str()).collect::<Vec<_>>(), ["itm_o"]);
    assert!(!is_pvp);
    assert_eq!(*opponent_name, None);
    let start_id = events[0].id.clone();

    h.memory.set_combat_result(sim(0, 1, 659));
    h.state("CombatState", "ReplayState");
    let ends = h.events();
    assert_eq!(names(&ends), ["CombatEnd"]);
    let (outcome, started, player_hp, opponent_hp, ..) = combat_end_of(&ends[0]);
    assert_eq!((*outcome, started, player_hp, opponent_hp), (Outcome::Win, Some(start_id.as_str()), Some(120), Some(0)));
    let EventKind::CombatEnd { sim_winner, sim_loser, sim_frames, expected_sim_frames, sim_diag, .. } = &ends[0].kind else { panic!() };
    assert_eq!((*sim_winner, *sim_loser, *sim_frames, *expected_sim_frames, sim_diag.as_deref()), (Some(0), Some(1), Some(659), Some(659), None));
}

#[test]
fn a_pvp_combat_carries_the_opponent_and_counts_the_win_before_memory_does() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 250, 1, 0));
    h.memory.set_pvp_opponent(PvpOpponent { name: Some("Rival".into()), hero: Some("Vanessa".into()), rating: Some(1180), level: Some(9) });
    h.start_run("run-1");
    h.events();

    h.state("ChoiceState", "PVPCombatState");
    h.advance(350);
    h.raw("[CombatSimHandler] Combat prepared: frames=415, effectsMs=0");
    let events = h.events();
    let EventKind::CombatStart { is_pvp, opponent_name, opponent_hero, opponent_rating, opponent_level, .. } = &events[0].kind else { panic!() };
    assert!(is_pvp);
    assert_eq!((opponent_name.as_deref(), opponent_hero.as_deref(), *opponent_rating, *opponent_level), (Some("Rival"), Some("Vanessa"), Some(1180), Some(9)));

    // The player wins, but the win counter has not been updated in memory yet.
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 90, 0, 1, 0));
    h.memory.set_combat_result(sim(0, 1, 415));
    h.state("PVPCombatState", "ReplayState");
    let ends = h.events();
    let (outcome, _, _, _, victories, losses) = combat_end_of(&ends[0]);
    assert_eq!((*outcome, victories, losses), (Outcome::Win, Some(2), Some(0)));
}

#[test]
fn a_card_placed_with_a_pvp_prefix_makes_the_fight_pvp() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 250, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.placed("pvp_9i8982w", "OpponentSocket_5", "Opponent");
    h.events();
    h.raw(PREPARED);
    let EventKind::CombatStart { is_pvp, .. } = &h.events()[0].kind else { panic!() };
    assert!(is_pvp);
}

#[test]
fn an_undecided_outcome_is_resolved_by_the_retry_after_1200_ms() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.advance(350);
    h.raw(PREPARED);
    h.events();

    // Neither the simulation nor the HP says yet.
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.state("CombatState", "ReplayState");
    assert!(h.events().is_empty());
    h.advance(1199);
    assert!(h.events().is_empty(), "not yet");
    // By the retry the opponent is dead.
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 0, 1, 0));
    h.advance(1);
    let ends = h.events();
    assert_eq!(names(&ends), ["CombatEnd"]);
    assert_eq!(*combat_end_of(&ends[0]).0, Outcome::Win);
}

#[test]
fn an_outcome_still_undecided_after_the_retry_is_sent_as_unknown() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.advance(350);
    h.raw(PREPARED);
    h.events();
    h.state("CombatState", "ReplayState");
    h.advance(1200);
    let ends = h.events();
    assert_eq!(names(&ends), ["CombatEnd"]);
    assert_eq!(*combat_end_of(&ends[0]).0, Outcome::Unknown);
    assert_eq!(ends[0].timestamp_ms, START_MS + 350 + 1200);
}

#[test]
fn the_win_and_loss_counters_decide_when_the_simulation_cannot() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.advance(350);
    h.raw(PREPARED);
    h.events();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 5, 80, 1, 1));
    h.state("CombatState", "ReplayState");
    assert_eq!(*combat_end_of(&h.events()[0]).0, Outcome::Loss);
}

#[test]
fn a_stale_simulation_is_ignored_and_says_so() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.advance(350);
    h.raw(PREPARED);
    h.events();
    // The simulation in memory is last fight's (100 frames, not 659) and says the player lost,
    // but the opponent's HP shows a win.
    h.memory.set_combat_result(sim(1, 0, 100));
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 0, 1, 0));
    h.state("CombatState", "ReplayState");
    let ends = h.events();
    assert_eq!(*combat_end_of(&ends[0]).0, Outcome::Win);
    let EventKind::CombatEnd { sim_diag, .. } = &ends[0].kind else { panic!() };
    assert_eq!(sim_diag.as_deref(), Some("stale (frame mismatch)"));
}

#[test]
fn leaving_combat_into_an_end_of_run_state_ends_the_fight_with_that_outcome_before_the_run() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.advance(350);
    h.raw(PREPARED);
    h.events();
    h.state("CombatState", "EndRunDefeatState");
    let events = h.events();
    assert_eq!(names(&events), ["CombatEnd", "RunEnded"]);
    assert_eq!(*combat_end_of(&events[0]).0, Outcome::Loss);
    let EventKind::RunEnded { outcome, .. } = &events[1].kind else { panic!() };
    assert_eq!(*outcome, Outcome::Loss);
}

#[test]
fn cards_spawning_before_the_fight_is_played_push_the_capture_back() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.advance(300);
    // More cards arrive: the board is not final, so the capture moves to 350 ms after this.
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o", "itm_o2"], 300, 200, 1, 0));
    h.raw("[GameSimHandler] GameSim cards: id=abc, dealt=0, spawned=1, disposed=0");
    h.advance(349);
    h.memory.set_board(fight_board(&["itm_late"], &[], 300, 200, 1, 0));
    h.advance(1);
    h.memory.set_board(fight_board(&[], &[], 300, 200, 1, 0));
    h.raw(PREPARED);
    let events = h.events();
    let EventKind::CombatStart { player, opponent, .. } = &events[0].kind else { panic!() };
    assert_eq!(player.board.iter().map(|i| i.instance_id.as_str()).collect::<Vec<_>>(), ["itm_late"], "captured 350 ms after the spawn, at the moment of the second change");
    assert!(opponent.board.is_empty());
}

#[test]
fn starting_the_fight_cancels_a_capture_that_is_still_waiting() {
    let mut h = harness();
    h.memory.set_board(fight_board(&["itm_a"], &["itm_o"], 300, 200, 1, 0));
    h.start_run("run-1");
    h.events();
    h.state("ChoiceState", "CombatState");
    h.advance(100);
    h.raw(PREPARED);
    let reads_after_start = h.memory.board_calls();
    assert_eq!(names(&h.events()), ["CombatStart"]);
    h.advance(1000);
    assert_eq!(h.memory.board_calls(), reads_after_start, "the 350 ms capture never ran");
}

#[test]
fn a_ranked_run_end_waits_for_the_rating_to_change() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.memory.set_player_rank(PlayerRank { rating: 1000, rank: "Gold".into(), division: 2 });
    h.memory.set_run_init(RunInit { run_id: Some("run-1".into()), play_mode: Some("Ranked".into()), ..RunInit::default() });
    h.run_initialized();
    h.command("RerollCommand");
    h.events();

    h.state("LootState", "EndRunVictoryState");
    assert!(h.events().is_empty(), "the rating has not changed yet");
    h.advance(1000);
    assert!(h.events().is_empty());
    h.memory.set_player_rank(PlayerRank { rating: 1022, rank: "Gold".into(), division: 1 });
    h.advance(500);
    let events = h.events();
    assert_eq!(names(&events), ["RunEnded"]);
    let EventKind::RunEnded { rating_after, rank_division_after, .. } = &events[0].kind else { panic!() };
    assert_eq!((*rating_after, *rank_division_after), (Some(1022), Some(1)));
    assert_eq!(events[0].timestamp_ms, START_MS + 1500);
}

#[test]
fn a_ranked_run_end_gives_up_after_four_seconds() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.memory.set_player_rank(PlayerRank { rating: 1000, rank: "Gold".into(), division: 2 });
    h.memory.set_run_init(RunInit { run_id: Some("run-1".into()), play_mode: Some("ranked".into()), ..RunInit::default() });
    h.run_initialized();
    h.command("RerollCommand");
    h.events();
    h.state("LootState", "EndRunDefeatState");
    h.advance(3999);
    assert!(h.events().is_empty());
    h.advance(1);
    let events = h.events();
    assert_eq!(names(&events), ["RunEnded"]);
    let EventKind::RunEnded { rating_after, .. } = &events[0].kind else { panic!() };
    assert_eq!(*rating_after, Some(1000));
    assert_eq!(events[0].timestamp_ms, START_MS + 4000);
}

#[test]
fn an_unranked_run_end_does_not_wait() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.memory.set_player_rank(PlayerRank { rating: 1000, rank: "Gold".into(), division: 2 });
    h.start_run("run-1");
    h.command("RerollCommand");
    h.events();
    h.state("LootState", "EndRunDefeatState");
    assert_eq!(names(&h.events()), ["RunEnded"]);
}

#[test]
fn a_run_that_started_under_a_local_id_is_rebound_when_memory_provides_the_real_one() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.command("RerollCommand");
    let events = h.events();
    assert_eq!(names(&events), ["RunStarted", "Rerolled"]);
    let local = events[0].run_id.clone();
    assert!(local.starts_with("local-"));

    // Memory learns the run's id; the next line the tracker sees picks it up.
    h.memory.set_run_init(RunInit { run_id: Some("real-run-9".into()), play_mode: Some("Normal".into()), ..RunInit::default() });
    h.state("ChoiceState", "EncounterState");
    let outputs = h.outputs();
    assert_eq!(outputs, [Output::Rebound { old_run_id: local, new_run_id: "real-run-9".into() }]);
    assert_eq!(h.machine.run_id(), Some("real-run-9"));

    // Later Events are under the real id, and the run is not started again.
    h.command("RerollCommand");
    let later = h.events();
    assert_eq!(names(&later), ["Rerolled"]);
    assert_eq!(later[0].run_id, "real-run-9");
}

#[test]
fn a_real_run_id_is_not_rebound_and_an_ended_local_run_is_not_either() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.command("RerollCommand");
    h.events();
    h.state("LootState", "EndRunVictoryState");
    h.events();
    // The local run has ended; a real id appearing now is a different run, not its real id.
    h.memory.set_run_init(RunInit { run_id: Some("real-run-9".into()), ..RunInit::default() });
    h.run_initialized();
    let outputs = h.outputs();
    assert!(outputs.iter().all(|output| !matches!(output, Output::Rebound { .. })), "{outputs:?}");
    assert_eq!(h.machine.run_id(), Some("real-run-9"));
}
