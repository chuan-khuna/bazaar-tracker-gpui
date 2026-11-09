use crate::memory::PlayerRank;
use crate::run::test_support::*;
use crate::run::{EventKind, Outcome};

#[test]
fn a_run_starts_on_run_init_with_the_real_id_and_what_memory_knows() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.memory.set_player_rank(PlayerRank { rating: 1234, rank: "Gold".into(), division: 2 });
    h.start_run("run-1");
    assert_eq!(h.machine.run_id(), Some("run-1"));
    let events = h.events();
    assert_eq!(names(&events), ["RunStarted"]);
    assert_eq!(events[0].run_id, "run-1");
    assert_eq!(events[0].timestamp_ms, START_MS);
    assert_eq!(
        events[0].kind,
        EventKind::RunStarted {
            play_mode: Some("Normal".into()),
            build_id: Some("b1".into()),
            environment: Some("prod".into()),
            hero: Some("Jules".into()),
            rating: Some(1234),
            rank: Some("Gold".into()),
            rank_division: Some(2),
        }
    );
    assert_eq!((events[0].day, events[0].hour), (Some(2), Some(3)));
}

#[test]
fn a_run_without_a_known_id_starts_under_a_local_id_on_its_first_activity() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.run_initialized();
    assert!(h.machine.run_id().is_none(), "memory has no run id, so the run waits for activity");
    assert!(h.events().is_empty());
    h.command("RerollCommand");
    let events = h.events();
    assert_eq!(names(&events), ["RunStarted", "Rerolled"]);
    assert!(events[0].run_id.starts_with("local-"), "{}", events[0].run_id);
    assert_eq!(events[0].run_id, events[1].run_id);
    assert_eq!(h.machine.run_id(), Some(events[0].run_id.as_str()));
}

#[test]
fn options_settle_500_ms_after_entering_an_option_state() {
    let mut h = harness();
    let offered = [item("itm_a", "Bronze", "Small"), item("itm_b", "Silver", "Medium"), item("enc_c", "Bronze", "Unknown")];
    h.memory.set_board(board(&[], &offered, 10));
    h.start_run("run-1");
    h.events();

    h.dealt(3);
    h.state("StartRunAppState", "ChoiceState");
    h.advance(499);
    assert!(h.events().is_empty(), "not settled yet");
    h.advance(1);
    let events = h.events();
    assert_eq!(names(&events), ["OptionsPresented"]);
    let EventKind::OptionsPresented { context, options, snapshot } = &events[0].kind else { panic!() };
    assert_eq!(context, "ChoiceState");
    let summary: Vec<_> = options.iter().map(|o| (o.instance_id.as_str(), o.kind.as_str(), o.size.as_str(), o.tier.as_deref())).collect();
    assert_eq!(summary, [("itm_a", "Item", "Small", Some("Bronze")), ("itm_b", "Item", "Medium", Some("Silver")), ("enc_c", "Encounter", "Unknown", Some("Bronze"))]);
    assert_eq!(options[0].template_id.as_deref(), Some("tpl-itm_a"));
    assert_eq!(snapshot.stats.gold, Some(10));
    assert_eq!(events[0].timestamp_ms, START_MS + 500);
}

#[test]
fn a_new_deal_while_in_an_option_state_restarts_the_settle() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[item("itm_a", "Bronze", "Small")], 10));
    h.start_run("run-1");
    h.events();
    h.state("StartRunAppState", "ChoiceState");
    h.advance(300);
    h.dealt(1);
    h.advance(300);
    assert!(h.events().is_empty(), "the new deal pushed the settle back");
    h.advance(200);
    assert_eq!(names(&h.events()), ["OptionsPresented"]);
}

#[test]
fn options_already_reported_are_not_reported_again() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[item("itm_a", "Bronze", "Small")], 10));
    h.start_run("run-1");
    h.events();
    h.dealt(1);
    h.state("StartRunAppState", "ChoiceState");
    h.advance(500);
    assert_eq!(names(&h.events()), ["OptionsPresented"]);
    h.dealt(1);
    h.advance(500);
    assert!(h.events().is_empty());
}

#[test]
fn a_newer_line_cancels_a_pending_settle() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[item("itm_a", "Bronze", "Small")], 10));
    h.start_run("run-1");
    h.events();
    h.dealt(1);
    h.state("StartRunAppState", "ChoiceState");
    h.advance(200);
    // The game moves on to a state with no options before the settle fires.
    h.state("ChoiceState", "ReplayState");
    h.advance(2000);
    assert!(h.events().is_empty(), "the pending settle was cancelled");
}

#[test]
fn events_are_built_when_sent_so_their_snapshot_is_of_that_moment() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[item("itm_a", "Bronze", "Small")], 1));
    h.start_run("run-1");
    h.events();
    h.dealt(1);
    h.state("StartRunAppState", "ChoiceState");
    // The gold changes between the line and the moment the options settle.
    h.memory.set_board(board(&[], &[item("itm_a", "Bronze", "Small")], 2));
    h.advance(500);
    let events = h.events();
    let EventKind::OptionsPresented { snapshot, .. } = &events[0].kind else { panic!() };
    assert_eq!(snapshot.stats.gold, Some(2));
}

#[test]
fn a_selection_is_linked_to_the_options_it_came_from() {
    let mut h = harness();
    let offered = [item("itm_a", "Bronze", "Small"), item("itm_b", "Gold", "Large")];
    h.memory.set_board(board(&[], &offered, 10));
    h.start_run("run-1");
    h.events();
    h.dealt(2);
    h.state("StartRunAppState", "ChoiceState");
    h.advance(500);
    let options_id = h.events()[0].id.clone();

    h.memory.set_board(board(&[item("itm_b", "Gold", "Large")], &[], 4));
    h.placed("itm_b", "PlayerSocket_3", "Player");
    let events = h.events();
    assert_eq!(names(&events), ["Selected"]);
    let EventKind::Selected { card_kind, instance_id, template_id, tier, section, target, options_event_id, snapshot, merged_into, .. } = &events[0].kind else { panic!() };
    assert_eq!((card_kind.as_str(), instance_id.as_str()), ("Item", "itm_b"));
    assert_eq!(template_id.as_deref(), Some("abc-123"), "from the log line");
    assert_eq!(tier.as_deref(), Some("Gold"), "from memory");
    assert_eq!((section.as_deref(), target.as_deref()), (Some("Player"), Some("PlayerSocket_3")));
    assert_eq!(options_event_id.as_deref(), Some(options_id.as_str()));
    assert_eq!(snapshot.stats.gold, Some(4));
    assert_eq!(snapshot.board.len(), 1);
    assert_eq!(*merged_into, None);
}

#[test]
fn choosing_before_the_options_settle_is_still_a_selection_without_a_link() {
    let mut h = harness();
    let offered = [item("itm_a", "Bronze", "Small"), item("itm_b", "Gold", "Large")];
    h.memory.set_board(board(&[], &offered, 10));
    h.start_run("run-1");
    h.events();
    h.dealt(2);
    h.state("StartRunAppState", "ChoiceState");
    h.advance(100);
    h.placed("itm_a", "PlayerSocket_0", "Player");
    let events = h.events();
    assert_eq!(names(&events), ["Selected"]);
    let EventKind::Selected { options_event_id, .. } = &events[0].kind else { panic!() };
    assert_eq!(*options_event_id, None, "the options had not been reported yet");
    h.advance(400);
    assert_eq!(names(&h.events()), ["OptionsPresented"]);
}

#[test]
fn a_skill_selection_is_a_selected_event_for_the_skill() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.events();
    h.raw("[AppState] Selected skill skl_tVCPvxR to socket SkillSocket_0");
    // A second report of the same skill is not a second selection.
    h.raw("[AppState] Selected skill skl_tVCPvxR to socket SkillSocket_0");
    let events = h.events();
    assert_eq!(names(&events), ["Selected"]);
    let EventKind::Selected { card_kind, instance_id, target, template_id, .. } = &events[0].kind else { panic!() };
    assert_eq!((card_kind.as_str(), instance_id.as_str(), target.as_deref()), ("Skill", "skl_tVCPvxR", Some("SkillSocket_0")));
    assert_eq!(*template_id, None);
}

#[test]
fn a_sale_is_the_item_that_left_the_players_board() {
    let mut h = harness();
    let owned = [item("itm_x", "Silver", "Small"), item("itm_y", "Bronze", "Small")];
    h.memory.set_board(board(&owned, &[], 10));
    h.start_run("run-1");
    h.events();

    h.memory.set_board(board(&owned[1..], &[], 11));
    h.command("SellCardCommand");
    let events = h.events();
    assert_eq!(names(&events), ["Sold"]);
    let EventKind::Sold { instance_id, tier, template_id, snapshot, .. } = &events[0].kind else { panic!() };
    assert_eq!(instance_id, "itm_x");
    assert_eq!(tier.as_deref(), Some("Silver"));
    assert_eq!(template_id.as_deref(), Some("tpl-itm_x"));
    assert_eq!(snapshot.stats.gold, Some(11));

    // The same sale is not reported again.
    h.command("SellCardCommand");
    assert!(h.events().is_empty());
}

#[test]
fn a_failed_command_is_ignored() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.events();
    h.raw("[HttpGameClient] Command completed: type=RerollCommand, requestId=1, status=400, result=failure, endpoint=/commands");
    assert!(h.events().is_empty());
}

#[test]
fn a_reroll_is_an_event() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.events();
    h.command("RerollCommand");
    assert_eq!(names(&h.events()), ["Rerolled"]);
}

#[test]
fn buying_a_duplicate_merges_the_offered_copy_that_disappeared() {
    let mut h = harness();
    let owned = [item("itm_c", "Bronze", "Small")];
    let offered = [item("itm_a", "Bronze", "Small"), item("itm_b", "Bronze", "Small")];
    h.memory.set_board(board(&owned, &offered, 10));
    h.start_run("run-1");
    h.events();
    h.dealt(2);
    h.state("StartRunAppState", "ChoiceState");
    h.advance(500);
    h.events();

    // The player buys a copy of itm_c: the offered itm_a vanishes and itm_c is upgraded.
    h.memory.set_board(board(&owned, &offered[1..], 5));
    h.placed("itm_c", "PlayerSocket_0", "Player");
    let events = h.events();
    assert_eq!(names(&events), ["Selected"]);
    let EventKind::Selected { instance_id, merged_into, .. } = &events[0].kind else { panic!() };
    assert_eq!(instance_id, "itm_a");
    assert_eq!(merged_into.as_deref(), Some("itm_c"));
}

#[test]
fn the_run_ends_on_the_end_of_run_states() {
    for (state, outcome) in [("EndRunVictoryState", Outcome::Win), ("EndRunDefeatState", Outcome::Loss)] {
        let mut h = harness();
        h.memory.set_board(board(&[], &[], 17));
        h.start_run("run-1");
        h.command("RerollCommand");
        h.events();
        h.state("LootState", state);
        let events = h.events();
        assert_eq!(names(&events), ["RunEnded"], "{state}");
        let EventKind::RunEnded { outcome: got, victories, losses, gold, health_max, .. } = &events[0].kind else { panic!() };
        assert_eq!(*got, outcome);
        assert_eq!((*victories, *losses, *gold, *health_max), (Some(1), Some(0), Some(17), Some(300)));
        assert!(h.machine.run_has_ended());
        // Nothing more happens for the same run.
        h.state(state, state);
        assert!(h.events().is_empty());
    }
}

#[test]
fn an_abandoned_run_ends_as_a_loss() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.command("RerollCommand");
    h.events();
    h.command("AbandonRunCommand");
    let events = h.events();
    let EventKind::RunEnded { outcome, .. } = &events[0].kind else { panic!() };
    assert_eq!(*outcome, Outcome::Loss);
}

#[test]
fn a_run_with_no_activity_does_not_end() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.events();
    h.state("StartRunAppState", "EndRunDefeatState");
    assert!(h.events().is_empty());
}

#[test]
fn a_different_run_id_ends_the_old_run_and_starts_a_new_one() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.command("RerollCommand");
    h.events();
    h.start_run("run-2");
    let events = h.events();
    assert_eq!(names(&events), ["RunEnded", "RunStarted"]);
    assert_eq!((events[0].run_id.as_str(), events[1].run_id.as_str()), ("run-1", "run-2"));
    let EventKind::RunEnded { outcome, .. } = &events[0].kind else { panic!() };
    assert_eq!(*outcome, Outcome::Unknown);
    assert_eq!(h.machine.run_id(), Some("run-2"));
}

#[test]
fn the_run_start_waits_for_the_hero_and_holds_back_what_follows() {
    let mut h = harness();
    let mut no_hero = board(&[], &[], 10);
    no_hero.player.stats.hero = None;
    h.memory.set_board(no_hero);
    h.start_run("run-1");
    assert!(h.events().is_empty(), "waiting for the hero");
    h.command("RerollCommand");
    assert!(h.events().is_empty(), "the reroll waits behind the run start");

    h.memory.set_selected_hero("Vanessa".to_owned());
    h.advance(500);
    let events = h.events();
    assert_eq!(names(&events), ["RunStarted", "Rerolled"]);
    let EventKind::RunStarted { hero, .. } = &events[0].kind else { panic!() };
    assert_eq!(hero.as_deref(), Some("Vanessa"));
}

#[test]
fn the_run_start_gives_up_waiting_for_the_hero_after_three_seconds() {
    let mut h = harness();
    let mut no_hero = board(&[], &[], 10);
    no_hero.player.stats.hero = None;
    h.memory.set_board(no_hero);
    h.start_run("run-1");
    for _ in 0..5 {
        h.advance(500);
    }
    assert!(h.events().is_empty());
    h.advance(500);
    let events = h.events();
    assert_eq!(names(&events), ["RunStarted"]);
    let EventKind::RunStarted { hero, .. } = &events[0].kind else { panic!() };
    assert_eq!(*hero, None);
    assert_eq!(events[0].timestamp_ms, START_MS + 3000);
}

#[test]
fn a_missing_day_and_hour_are_filled_from_the_last_board_read() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.events();
    // The next board read fails, but the last day and hour are remembered.
    h.memory.fail_board("boom");
    h.command("RerollCommand");
    let events = h.events();
    assert_eq!((events[0].day, events[0].hour), (Some(2), Some(3)));
}

#[test]
fn each_event_has_its_own_id() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.command("RerollCommand");
    h.command("RerollCommand");
    let ids: Vec<_> = h.events().into_iter().map(|event| event.id).collect();
    assert_eq!(ids, ["e1", "e2", "e3"]);
}

#[test]
fn reset_forgets_the_run() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.events();
    h.machine.reset();
    assert!(h.machine.run_id().is_none());
    assert!(h.machine.next_due_ms().is_none());
}

fn run_init(run_id: Option<&str>) -> crate::memory::RunInit {
    crate::memory::RunInit { run_id: run_id.map(str::to_owned), play_mode: Some("Normal".into()), ..Default::default() }
}

#[test]
fn picking_up_follows_the_run_being_played_without_a_log_line() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.memory.set_run_init(run_init(Some("run-1")));
    assert_eq!(h.machine.pick_up(), crate::run::PickUp::Following("run-1".into()));
    let events = h.events();
    assert_eq!(names(&events), ["RunStarted"]);
    assert_eq!((events[0].run_id.as_str(), events[0].day, events[0].hour), ("run-1", Some(2), Some(3)));
    assert_eq!(h.machine.run_id(), Some("run-1"));
    // Again: nothing new.
    assert_eq!(h.machine.pick_up(), crate::run::PickUp::AlreadyFollowing("run-1".into()));
    assert!(h.events().is_empty());
    // It was being played, so its end is saved even with nothing happening since.
    h.state("LootState", "EndRunDefeatState");
    assert_eq!(names(&h.events()), ["RunEnded"]);
}

#[test]
fn picking_up_leaves_alone_no_run_a_run_that_is_over_and_a_run_with_no_id() {
    let mut h = harness();
    assert_eq!(h.machine.pick_up(), crate::run::PickUp::NoRun);

    let mut over = board(&[], &[], 10);
    over.run_state = Some("EndRunVictory".into());
    h.memory.set_board(over);
    h.memory.set_run_init(run_init(Some("run-1")));
    assert_eq!(h.machine.pick_up(), crate::run::PickUp::RunOver);

    h.memory.set_board(board(&[], &[], 10));
    h.memory.set_run_init(run_init(None));
    assert_eq!(h.machine.pick_up(), crate::run::PickUp::NoRunId);
    assert!(h.events().is_empty());
    assert!(h.machine.run_id().is_none());
}

#[test]
fn after_a_run_ends_picking_up_takes_a_new_run_but_never_the_ended_one() {
    let mut h = harness();
    h.memory.set_board(board(&[], &[], 10));
    h.start_run("run-1");
    h.command("RerollCommand");
    h.state("LootState", "EndRunDefeatState");
    h.events();
    // Memory still names the ended run.
    assert_eq!(h.machine.pick_up(), crate::run::PickUp::RunOver);
    assert!(h.events().is_empty());
    // A new run in the game, its run-init line missed.
    h.memory.set_run_init(run_init(Some("run-2")));
    assert_eq!(h.machine.pick_up(), crate::run::PickUp::Following("run-2".into()));
    assert_eq!(names(&h.events()), ["RunStarted"]);
}
