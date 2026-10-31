use bazaar_tracker::memory::{ConnectionChange, PlayerStats, Snapshot};

use super::*;
use crate::tracking::test_events::item;

fn board(state: &str, offered: &[&str]) -> BoardState {
    let mut mine = item("mine", "Medium", Some("Socket_2"));
    mine.template_id = Some("tpl-mine".into());
    BoardState {
        player: Snapshot { board: vec![mine], stash: vec![item("held", "Small", None)], skills: vec![item("skill", "Small", None)], stats: PlayerStats { day: Some(2), hour: Some(1), gold: Some(9), ..PlayerStats::default() } },
        run_state: Some(state.into()),
        selection_set: offered.iter().map(|id| (*id).to_owned()).collect(),
        entities: offered
            .iter()
            .map(|id| {
                let mut entity = item(id, "Small", None);
                if id.starts_with("enc_") {
                    entity.card_type = "CombatEncounter".into();
                }
                entity
            })
            .collect(),
        ..BoardState::default()
    }
}

fn view(board: &BoardState) -> LiveView {
    live_view(board, &Names::default())
}

#[test]
fn a_shop_shows_its_items_as_tiles_and_ours() {
    let view = view(&board("Encounter", &["itm_a", "itm_b", "itm_c"]));
    assert_eq!(view.state, "Shop · Day 2 Hour 1");
    let offered = view.offered.unwrap();
    assert_eq!(offered.title, "SHOP");
    assert_eq!(offered.items.tiles.iter().map(|tile| (tile.name.as_str(), tile.start)).collect::<Vec<_>>(), [("tpl-itm_a", 0), ("tpl-itm_b", 1), ("tpl-itm_c", 2)]);
    assert!(offered.cards.is_empty());
    assert_eq!(view.mine.board.tiles.len(), 1);
    assert_eq!((view.mine.board.tiles[0].start, view.mine.board.tiles[0].span), (2, 2));
    assert_eq!((view.mine.stash.tiles.len(), view.mine.skills.len()), (1, 1));
    assert_eq!(view.mine.moment, "Shop · Day 2 Hour 1");
    assert_eq!(view.mine.stats.gold, Some(9));
    assert!(view.opponent.is_none());
}

#[test]
fn a_choice_shows_the_encounter_options_as_cards_by_name() {
    let view = view(&board("Choice", &["enc_x", "enc_y", "enc_z"]));
    assert_eq!(view.state, "Encounter choice · Day 2 Hour 1");
    let offered = view.offered.unwrap();
    assert_eq!(offered.title, "CHOICES");
    assert!(offered.items.tiles.is_empty());
    let names: Vec<_> = offered.cards.iter().map(|card| (card.name.as_str(), card.meta.as_str())).collect();
    assert_eq!(names, [("tpl-enc_x", "CombatEncounter · Gold"), ("tpl-enc_y", "CombatEncounter · Gold"), ("tpl-enc_z", "CombatEncounter · Gold")]);
    assert_eq!(view.mine.board.tiles.len(), 1, "our items are still shown");
}

#[test]
fn an_option_the_read_has_no_entity_for_is_shown_by_its_id() {
    let mut read = board("Choice", &["enc_x"]);
    read.entities.clear();
    let offered = view(&read).offered.unwrap();
    assert_eq!((offered.cards[0].name.as_str(), offered.cards[0].meta.as_str()), ("enc_x", ""));
}

#[test]
fn a_shop_item_the_read_has_no_entity_for_is_not_a_tile() {
    let mut read = board("Encounter", &["itm_a"]);
    read.entities.clear();
    let offered = view(&read).offered.unwrap();
    assert!(offered.items.tiles.is_empty());
    assert_eq!(offered.cards[0].name, "itm_a");
}

#[test]
fn a_combat_shows_the_opponents_board_if_it_was_read() {
    let mut read = board("PVPCombat", &[]);
    read.opponent.board = vec![item("o1", "Small", Some("Socket_0")), item("o2", "Medium", Some("Socket_1"))];
    let view = view(&read);
    assert_eq!(view.state, "Combat · Day 2 Hour 1");
    assert!(view.offered.is_none());
    assert_eq!(view.opponent.unwrap().tiles.len(), 2);

    let mut unread = board("Combat", &[]);
    unread.opponent = Snapshot::default();
    assert!(self::view(&unread).opponent.is_none());
}

#[test]
fn the_opponents_last_board_is_not_shown_outside_a_combat() {
    // The game keeps the last opponent's values while in a shop (record 2026-10-31.002).
    let mut read = board("Encounter", &[]);
    read.opponent.board = vec![item("o1", "Small", None)];
    assert!(view(&read).opponent.is_none());
}

#[test]
fn another_state_is_named_as_the_game_names_it() {
    let view = view(&board("Loot", &["itm_a"]));
    assert_eq!(view.state, "Loot · Day 2 Hour 1");
    assert_eq!(view.offered.unwrap().title, "ON OFFER");
    assert_eq!(live_view(&board("LevelUp", &[]), &Names::default()).state, "LevelUp · Day 2 Hour 1");
}

#[test]
fn a_read_without_a_state_is_still_shown() {
    let mut read = board("x", &[]);
    read.run_state = None;
    assert_eq!(view(&read).state, "Unknown state · Day 2 Hour 1");
}

#[test]
fn the_section_needs_the_game_attached_and_a_live_run() {
    let mut live = LiveState::default();
    live.apply(&Notice::LiveSnapshot(Some(board("Encounter", &[]))));
    let names = Names::default();
    assert!(matches!(live.view(false, true, &names), Live::Placeholder(text) if text.starts_with("Waiting")));
    assert!(matches!(live.view(true, false, &names), Live::Placeholder(text) if text.starts_with("No run")));
    assert!(matches!(live.view(true, true, &names), Live::View(_)));
    assert!(matches!(LiveState::default().view(true, true, &names), Live::Placeholder(_)));
}

#[test]
fn each_read_replaces_the_last_and_no_board_or_losing_the_game_clears_it() {
    let mut live = LiveState::default();
    let names = Names::default();
    let state = |live: &LiveState| match live.view(true, true, &names) {
        Live::View(view) => Some(view.state),
        Live::Placeholder(_) => None,
    };
    live.apply(&Notice::LiveSnapshot(Some(board("Encounter", &[]))));
    assert_eq!(state(&live).as_deref(), Some("Shop · Day 2 Hour 1"));
    live.apply(&Notice::LiveSnapshot(Some(board("Combat", &[]))));
    assert_eq!(state(&live).as_deref(), Some("Combat · Day 2 Hour 1"));
    live.apply(&Notice::LiveSnapshot(None));
    assert_eq!(state(&live), None);
    live.apply(&Notice::LiveSnapshot(Some(board("Encounter", &[]))));
    live.apply(&Notice::Connection(ConnectionChange::Disconnected { reason: "gone".into() }));
    assert_eq!(state(&live), None);
    live.apply(&Notice::LiveSnapshot(Some(board("Encounter", &[]))));
    live.apply(&Notice::LogError("unrelated".into()));
    assert!(state(&live).is_some(), "other notices leave it alone");
}
