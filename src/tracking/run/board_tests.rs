use std::sync::Arc;

use bazaar_tracker::catalog::CardCatalog;
use bazaar_tracker::memory::PlayerStats;

use super::*;
use crate::tracking::test_events::*;

fn starts(grid: &Grid) -> Vec<(String, u8, u8)> {
    grid.tiles.iter().map(|tile| (tile.name.clone(), tile.start, tile.span)).collect()
}

fn with_attributes(mut item: Item, attributes: &[(&str, i32)]) -> Item {
    item.attributes = attributes.iter().map(|(name, value)| ((*name).to_owned(), *value)).collect();
    item
}

#[test]
fn items_sit_in_their_sockets_and_cover_their_size() {
    let items = [item("a", "Small", Some("Socket_7")), item("b", "Medium", Some("Socket_0")), item("c", "Large", Some("Socket_3"))];
    let grid = board_grid(&items, &Names::default());
    assert_eq!(starts(&grid), [("tpl-b".into(), 0, 2), ("tpl-c".into(), 3, 3), ("tpl-a".into(), 7, 1)]);
    assert_eq!(grid.hidden, 0);
    assert_eq!(grid.used_slots(), 6);
}

#[test]
fn an_item_whose_socket_is_taken_goes_to_the_first_gap_it_fits() {
    let items = [item("a", "Medium", Some("Socket_0")), item("b", "Medium", Some("Socket_1")), item("c", "Small", None)];
    let grid = board_grid(&items, &Names::default());
    assert_eq!(starts(&grid), [("tpl-a".into(), 0, 2), ("tpl-b".into(), 2, 2), ("tpl-c".into(), 4, 1)]);
}

#[test]
fn an_item_running_off_the_end_is_moved_to_a_gap() {
    let items = [item("a", "Large", Some("Socket_9"))];
    assert_eq!(starts(&board_grid(&items, &Names::default())), [("tpl-a".into(), 0, 3)]);
}

#[test]
fn a_socket_wanted_by_one_item_is_not_taken_by_another_that_came_first() {
    // `a` has no socket and is first, but `b` names slot 0.
    let items = [item("a", "Small", None), item("b", "Small", Some("Socket_0"))];
    assert_eq!(starts(&board_grid(&items, &Names::default())), [("tpl-b".into(), 0, 1), ("tpl-a".into(), 1, 1)]);
}

#[test]
fn items_that_fit_nowhere_are_counted() {
    let items: Vec<Item> = (0..4).map(|n| item(&format!("i{n}"), "Large", None)).collect();
    let grid = board_grid(&items, &Names::default());
    assert_eq!(grid.tiles.len(), 3);
    assert_eq!(grid.hidden, 1);
}

#[test]
fn socket_effects_are_not_items() {
    let mut effect = item("e", "Small", Some("Socket_6"));
    effect.card_type = "SocketEffect".into();
    let grid = board_grid(&[effect, item("a", "Small", Some("Socket_6"))], &Names::default());
    assert_eq!(starts(&grid), [("tpl-a".into(), 6, 1)]);
    assert_eq!(grid.marks.iter().map(|mark| mark.slot).collect::<Vec<_>>(), [6]);
}

#[test]
fn stoves_coolers_and_notes_mark_their_one_slot() {
    let marks = [(9, "Stove"), (2, "Cooler"), (4, "B Note (Burn)"), (7, "Something New")].map(|(slot, name)| SlotMark::new(slot, name.to_owned()));
    let seen: Vec<_> = marks.iter().map(|mark| (mark.slot, mark.kind, mark.label.as_str())).collect();
    assert_eq!(seen, [(9, SlotKind::Heat, "Heat"), (2, SlotKind::Chill, "Chill"), (4, SlotKind::Note, "B"), (7, SlotKind::Other, "Something New")]);
}

#[test]
fn the_board_lists_its_marks_in_slot_order_and_drops_those_off_the_board() {
    let effect = |id: &str, socket: Option<&str>| {
        let mut effect = item(id, "Small", socket);
        effect.card_type = "SocketEffect".into();
        effect
    };
    let grid = board_grid(&[effect("x", Some("Socket_8")), effect("y", Some("Socket_1")), effect("z", None), effect("w", Some("Socket_12"))], &Names::default());
    assert_eq!(grid.marks.iter().map(|mark| mark.slot).collect::<Vec<_>>(), [1, 8]);
}

#[test]
fn the_stash_is_laid_out_in_order_whatever_its_sockets() {
    let items = [item("a", "Medium", Some("Socket_5")), item("b", "Small", Some("Socket_5"))];
    assert_eq!(starts(&stash_grid(&items, &Names::default())), [("tpl-a".into(), 0, 2), ("tpl-b".into(), 2, 1)]);
}

#[test]
fn tiles_show_the_name_tier_and_enchantment() {
    let cards = r#"{ "Items": [ { "Id": "tpl-a", "Localization": { "Title": { "Text": "Cheese Wheel" } } } ] }"#;
    let names = Names::new(Arc::new(CardCatalog::from_cards_json(cards).unwrap()));
    let mut cheese = item("a", "Medium", Some("Socket_0"));
    cheese.enchantment = Some("Mossy".into());
    let tile = &board_grid(&[cheese], &names).tiles[0];
    assert_eq!((tile.name.as_str(), tile.enchantment.as_deref(), tile.tier), ("Cheese Wheel", Some("Mossy"), Some(Tier::Gold)));
}

#[test]
fn badges_come_from_attributes_above_zero_and_the_prices_have_their_own_corners() {
    let weapon = with_attributes(
        item("a", "Small", Some("Socket_0")),
        &[
            ("DamageAmount", 30),
            ("ShieldApplyAmount", 5),
            ("HealAmount", 6),
            ("BurnApplyAmount", 7),
            ("PoisonApplyAmount", 8),
            ("RegenApplyAmount", 9),
            ("AmmoMax", 4),
            ("Ammo", 3),
            ("Multicast", 2),
            ("SellPrice", 4),
            ("CooldownMax", 4500),
            ("Cooldown", 1000),
            ("Lifesteal", 0),
            ("CritChance", 25),
        ],
    );
    let tile = &board_grid(&[weapon], &Names::default()).tiles[0];
    let badges: Vec<_> = tile.badges.iter().map(|badge| (badge.kind, badge.text.as_str())).collect();
    assert_eq!(
        badges,
        [
            (BadgeKind::Damage, "30"),
            (BadgeKind::Shield, "5"),
            (BadgeKind::Heal, "6"),
            (BadgeKind::Burn, "7"),
            (BadgeKind::Poison, "8"),
            (BadgeKind::Regen, "9"),
            (BadgeKind::Ammo, "3"),
            (BadgeKind::Multicast, "x2"),
        ]
    );
    assert_eq!(tile.sell_price, Some(4));
    assert_eq!(tile.cooldown.as_deref(), Some("4.5s"));
}

#[test]
fn zero_attributes_make_no_badges_and_a_single_cast_is_not_a_badge() {
    let plain = with_attributes(item("a", "Small", Some("Socket_0")), &[("DamageAmount", 0), ("Multicast", 1), ("AmmoMax", 0), ("Ammo", 5), ("SellPrice", 0), ("CooldownMax", 0)]);
    let tile = &board_grid(&[plain], &Names::default()).tiles[0];
    assert!(tile.badges.is_empty());
    assert_eq!((tile.sell_price, tile.cooldown.as_deref()), (None, None));
}

#[test]
fn a_whole_number_of_seconds_has_no_decimal() {
    let slow = with_attributes(item("a", "Small", None), &[("CooldownMax", 3000)]);
    assert_eq!(board_grid(&[slow], &Names::default()).tiles[0].cooldown.as_deref(), Some("3s"));
}

#[test]
fn the_tooltip_has_everything_the_tile_does_not_show() {
    let mut food = with_attributes(item("a", "Small", Some("Socket_0")), &[("100", 1), ("DamageAmount", 30), ("SellPrice", 4), ("CooldownMax", 3000), ("Chilled", 1), ("Haste", 0), ("Multicast", 1)]);
    food.enchantment = Some("Toxic".into());
    food.tags = vec!["Food".into(), "Tool".into()];
    let tile = &board_grid(&[food], &Names::default()).tiles[0];
    assert_eq!(tile.tooltip, ["Gold · Small · Item", "Enchantment: Toxic", "Tags: Food, Tool", "100: 1", "Chilled: 1", "Multicast: 1"]);
}

#[test]
fn skills_are_chips_with_a_tier_and_a_tooltip() {
    let mut skill = item("s", "Small", None);
    skill.card_type = "Skill".into();
    skill.tier = "Silver".into();
    let snapshot = Snapshot { skills: vec![skill], ..Snapshot::default() };
    let events = [options("o", 0, snapshot)];
    let chips = view(&events, Choice::Latest, true, &Names::default()).skills;
    assert_eq!(chips.len(), 1);
    assert_eq!((chips[0].name.as_str(), chips[0].tier), ("tpl-s", Some(Tier::Silver)));
    assert_eq!(chips[0].tooltip, ["Silver · Small · Skill"]);
}

#[test]
fn skills_are_sorted_rarest_first_keeping_the_games_order_within_a_tier() {
    let skill = |id: &str, tier: &str| {
        let mut skill = item(id, "Small", None);
        skill.tier = tier.into();
        skill
    };
    let skills = [skill("a", "Bronze"), skill("b", "Gold"), skill("c", ""), skill("d", "Legendary"), skill("e", "Gold"), skill("f", "Silver")];
    let names: Vec<String> = skill_chips(&skills, &Names::default()).into_iter().map(|chip| chip.name).collect();
    assert_eq!(names, ["tpl-d", "tpl-b", "tpl-e", "tpl-f", "tpl-a", "tpl-c"]);
}

#[test]
fn the_view_names_the_moment_and_shows_the_stats() {
    let mut snap = snapshot(14, &["a"]);
    snap.stats = PlayerStats { health: Some(1240), health_max: Some(1400), gold: Some(14), income: Some(5), level: Some(9), day: Some(7), hour: Some(3), ..PlayerStats::default() };
    let events = [options("o1", 0, snap), sold("o2", 10, snapshot(15, &[]))];
    let live = view(&events[..1], Choice::Latest, true, &Names::default());
    assert_eq!((live.moment.as_str(), live.time.as_deref()), ("live", Some("Day 7 · Hour 3")));
    assert_eq!(live.stats, Stats { health: Some("1240 / 1400".into()), gold: Some("14 (+5)".into()), level: Some(9) });
    let done = view(&events[..1], Choice::Latest, false, &Names::default());
    assert_eq!((done.moment.as_str(), done.time.as_deref()), ("end of run", Some("Day 7 · Hour 3")));
    let at = view(&events, Choice::Event("o1"), false, &Names::default());
    assert_eq!((at.moment.as_str(), at.time.as_deref()), ("Event #1", Some("Day 7 · Hour 3")));    assert_eq!(at.stats.gold.as_deref(), Some("14 (+5)"));
}

#[test]
fn a_run_with_no_snapshot_has_an_empty_board() {
    let view = view(&[], Choice::Latest, true, &Names::default());
    assert_eq!(view.moment, "no board yet");
    assert!(view.board.tiles.is_empty() && view.skills.is_empty());
    assert_eq!(view.stash_summary, "0 items · 0 of 10 slots");
}

#[test]
fn the_stash_summary_counts_items_and_slots() {
    let snap = Snapshot { stash: vec![item("a", "Medium", None), item("b", "Small", None)], ..Snapshot::default() };
    let view = view(&[options("o", 0, snap)], Choice::Latest, true, &Names::default());
    assert_eq!(view.stash_summary, "2 items · 3 of 10 slots");
    let one = Snapshot { stash: vec![item("a", "Small", None)], ..Snapshot::default() };
    assert_eq!(super::view(&[options("o", 0, one)], Choice::Latest, true, &Names::default()).stash_summary, "1 item · 1 of 10 slots");
}

#[test]
fn gold_shows_the_income_beside_it_when_it_was_read() {
    let gold = |income: Option<i32>| {
        let mut snap = snapshot(14, &[]);
        snap.stats = PlayerStats { gold: Some(36), income, ..PlayerStats::default() };
        view_of(&snap, String::new(), None, &Names::default()).stats.gold
    };
    assert_eq!(gold(Some(5)).as_deref(), Some("36 (+5)"));
    assert_eq!(gold(Some(0)).as_deref(), Some("36 (+0)"));
    assert_eq!(gold(Some(-2)).as_deref(), Some("36 (-2)"));
    assert_eq!(gold(None).as_deref(), Some("36"));
}
