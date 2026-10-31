use super::*;
use crate::tracking::test_events::*;

fn option(id: &str, tier: &str) -> PresentedOption {
    PresentedOption { instance_id: id.into(), kind: "Item".into(), size: "Small".into(), template_id: Some(format!("tpl-{id}")), tier: Some(tier.into()), enchantment: None }
}

fn options_event(id: &str) -> Event {
    event(id, 0, EventKind::OptionsPresented { context: "ChoiceState".into(), options: vec![option("a", "Bronze"), option("b", "Gold"), option("c", "Silver")], snapshot: Snapshot::default() })
}

fn selected(id: &str, instance: &str, from: Option<&str>) -> Event {
    event(
        id,
        0,
        EventKind::Selected {
            card_kind: "Item".into(),
            instance_id: instance.into(),
            template_id: Some(format!("tpl-{instance}")),
            tier: Some("Gold".into()),
            enchantment: None,
            section: Some("Hand".into()),
            target: None,
            merged_into: None,
            options_event_id: from.map(str::to_owned),
            snapshot: Snapshot::default(),
        },
    )
}

fn cards(view: &DetailView, section: usize) -> Vec<(&str, bool)> {
    view.sections[section].cards.iter().map(|card| (card.name.as_str(), card.picked)).collect()
}

fn detail_of(events: &[Event], id: &str) -> DetailView {
    detail(events, id, &Names::default())
}

#[test]
fn options_show_every_card_with_the_pick_marked() {
    let events = [options_event("o"), selected("s", "b", Some("o"))];
    let view = detail_of(&events, "o");
    assert!(view.title.starts_with("Options · #1 · "));
    assert_eq!(view.banner.as_deref(), Some("Picked 1 of 3"));
    assert_eq!(view.sections[0].title, "OPTIONS");
    assert_eq!(cards(&view, 0), [("tpl-a", false), ("tpl-b", true), ("tpl-c", false)]);
    assert_eq!(view.sections[0].cards[1].meta, "Item · Gold");
    assert_eq!(view.sections[0].cards[1].tier, Some(Tier::Gold));
}

#[test]
fn options_with_no_pick_yet_say_so() {
    let view = detail_of(&[options_event("o")], "o");
    assert_eq!(view.banner.as_deref(), Some("3 offered, none picked"));
    assert!(view.sections[0].cards.iter().all(|card| !card.picked));
}

#[test]
fn a_pick_made_from_another_options_event_is_not_counted() {
    let events = [options_event("o1"), options_event("o2"), selected("s", "a", Some("o2"))];
    assert_eq!(detail_of(&events, "o1").banner.as_deref(), Some("3 offered, none picked"));
    assert_eq!(detail_of(&events, "o2").banner.as_deref(), Some("Picked 1 of 3"));
}

#[test]
fn a_selection_shows_the_options_it_was_chosen_from() {
    let events = [options_event("o"), selected("s", "c", Some("o"))];
    let view = detail_of(&events, "s");
    assert!(view.title.starts_with("Selected · #2 · "));
    assert_eq!(cards(&view, 0), [("tpl-a", false), ("tpl-b", false), ("tpl-c", true)]);
    assert_eq!(view.facts, [("Section", "Hand".to_owned())]);
}

#[test]
fn a_selection_whose_options_are_missing_shows_the_one_card() {
    let view = detail_of(&[selected("s", "c", Some("gone"))], "s");
    assert_eq!(view.sections[0].title, "SELECTED");
    assert_eq!(cards(&view, 0), [("tpl-c", true)]);
    assert_eq!(view.banner, None);
}

#[test]
fn a_sale_shows_the_card_sold() {
    let view = detail_of(&[sold("s", 0, Snapshot::default())], "s");
    assert_eq!(view.sections[0].title, "SOLD");
    assert_eq!(cards(&view, 0), [("itm_x", false)]);
}

fn fighters() -> (Snapshot, Snapshot) {
    let mine = Snapshot { board: vec![item("m1", "Small", None)], ..Snapshot::default() };
    let mut effect = item("fx", "Small", None);
    effect.card_type = "SocketEffect".into();
    let mut theirs = item("t1", "Small", None);
    theirs.enchantment = Some("Toxic".into());
    theirs.tier = "Silver".into();
    (mine, Snapshot { board: vec![theirs, item("t2", "Medium", None), effect], ..Snapshot::default() })
}

#[test]
fn a_combat_start_shows_the_opponent_and_both_boards() {
    let (mine, theirs) = fighters();
    let start = event(
        "c",
        0,
        EventKind::CombatStart { player: mine, opponent: theirs, is_pvp: true, opponent_name: Some("Bob".into()), opponent_hero: Some("Mak".into()), opponent_rating: Some(900), opponent_level: Some(7) },
    );
    let view = detail_of(&[start], "c");
    assert_eq!(view.facts, [("Mode", "PvP".to_owned()), ("Opponent", "Bob".to_owned()), ("Hero", "Mak".to_owned()), ("Rating", "900".to_owned()), ("Level", "7".to_owned())]);
    assert_eq!(view.sections[0].title, "OPPONENT");
    assert_eq!(cards(&view, 0), [("tpl-t1", false), ("tpl-t2", false)], "socket effects are left out");
    assert_eq!(view.sections[0].cards[0].meta, "Silver · Toxic");
    assert_eq!(view.sections[1].title, "PLAYER");
    assert_eq!(cards(&view, 1), [("tpl-m1", false)]);
}

#[test]
fn a_combat_end_shows_the_result_and_the_opponent_board_of_its_start() {
    let (mine, theirs) = fighters();
    let start = combat_start("c", 0, mine, theirs);
    let end = event(
        "e",
        0,
        EventKind::CombatEnd {
            outcome: Outcome::Loss,
            combat_start_event_id: Some("c".into()),
            player_hp_after: Some(0),
            opponent_hp_after: Some(120),
            victories_after: Some(3),
            losses_after: Some(1),
            sim_winner: None,
            sim_loser: None,
            sim_frames: None,
            expected_sim_frames: None,
            sim_diag: Some("verdict from the log".into()),
        },
    );
    let view = detail_of(&[start, end], "e");
    assert_eq!(
        view.facts,
        [("Result", "Defeat".to_owned()), ("Your health", "0".to_owned()), ("Opponent health", "120".to_owned()), ("Record", "3W–1L".to_owned()), ("Note", "verdict from the log".to_owned())]
    );
    assert_eq!(view.sections.len(), 1);
    assert_eq!(cards(&view, 0), [("tpl-t1", false), ("tpl-t2", false)]);
}

#[test]
fn a_combat_end_without_its_start_has_no_board() {
    let view = detail_of(&[combat_end("e", 0, Outcome::Win)], "e");
    assert!(view.sections.is_empty());
    assert_eq!(view.facts, [("Result", "Victory".to_owned())]);
}

#[test]
fn run_started_and_ended_and_rerolled_show_their_facts() {
    let events = [
        event("s", 0, EventKind::RunStarted { play_mode: Some("Ranked".into()), build_id: None, environment: None, hero: Some("Mak".into()), rating: Some(812), rank: Some("Gold".into()), rank_division: Some(3) }),
        event("r", 0, EventKind::Rerolled),
        event(
            "e",
            0,
            EventKind::RunEnded {
                outcome: Outcome::Win,
                victories: Some(10),
                losses: Some(2),
                gold: Some(40),
                health_max: None,
                prestige: None,
                level: Some(9),
                income: None,
                rating_after: Some(836),
                rank_after: None,
                rank_division_after: None,
            },
        ),
    ];
    assert_eq!(detail_of(&events, "s").facts, [("Hero", "Mak".to_owned()), ("Mode", "Ranked".to_owned()), ("Rank", "Gold 3".to_owned()), ("Rating", "812".to_owned())]);
    assert_eq!(detail_of(&events, "r").banner.as_deref(), Some("The offers were refreshed."));
    assert_eq!(
        detail_of(&events, "e").facts,
        [("Result", "Victory".to_owned()), ("Record", "10W–2L".to_owned()), ("Gold", "40".to_owned()), ("Level", "9".to_owned()), ("Rating", "836".to_owned())]
    );
}

#[test]
fn an_event_that_is_gone_says_so() {
    let view = detail_of(&[options_event("o")], "nope");
    assert_eq!(view.banner.as_deref(), Some("This event is no longer in the run."));
    assert!(view.sections.is_empty() && view.facts.is_empty());
}
