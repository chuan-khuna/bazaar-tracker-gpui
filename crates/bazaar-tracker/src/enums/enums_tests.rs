use super::*;

#[test]
fn known_values_have_their_names() {
    assert_eq!(tier::name(0), Some("Bronze"));
    assert_eq!(tier::name(4), Some("Legendary"));
    assert_eq!(card_size::name(2), Some("Medium"));
    assert_eq!(card_type::name(1), Some("Skill"));
    assert_eq!(hero::name(4), Some("Jules"));
    assert_eq!(run_state::name(9), Some("PVPCombat"));
    assert_eq!(inventory_section::name(1), Some("Stash"));
    assert_eq!(container_socket::name(9), Some("Socket_9"));
    assert_eq!(card_tag::name(26), Some("Drone"));
    assert_eq!(card_attribute::name(38), Some("SellPrice"));
    assert_eq!(enchantment::name(15), Some("Mossy"));
    assert_eq!(player_attribute::name(10), Some("Health"));
    assert_eq!(rank::name(2), Some("Gold"));
    assert_eq!(combatant::name(1), Some("Opponent"));
}

#[test]
fn an_unknown_number_keeps_its_raw_value() {
    assert_eq!(tier::name(9), None);
    assert_eq!(tier::name_or_raw(9), "9");
    assert_eq!(tier::name_or_raw(-1), "-1");
    assert_eq!(tier::name_or_raw(2), "Gold");
    // The enchantment numbers have gaps at 1, 6 and 12.
    assert_eq!(enchantment::name(1), None);
    assert_eq!(enchantment::name_or_raw(6), "6");
    assert_eq!(card_size::name_or_raw(0), "0");
}

#[test]
fn names_map_back_to_their_values() {
    assert_eq!(hero::value_of("Pygmalien"), Some(1));
    assert_eq!(player_attribute::value_of("HealthMax"), Some(11));
    assert_eq!(card_type::value_of("SocketEffect"), Some(7));
    assert_eq!(hero::value_of("Nobody"), None);
}

#[test]
fn every_table_has_unique_values_and_names() {
    fn check(label: &str, values: &[(i64, &str)]) {
        for (i, (value, name)) in values.iter().enumerate() {
            for (other_value, other_name) in &values[i + 1..] {
                assert_ne!(value, other_value, "{label}: value {value} appears twice");
                assert_ne!(name, other_name, "{label}: name {name} appears twice");
            }
        }
    }
    check(tier::GAME_NAME, tier::VALUES);
    check(rank::GAME_NAME, rank::VALUES);
    check(card_size::GAME_NAME, card_size::VALUES);
    check(card_type::GAME_NAME, card_type::VALUES);
    check(hero::GAME_NAME, hero::VALUES);
    check(combatant::GAME_NAME, combatant::VALUES);
    check(inventory_section::GAME_NAME, inventory_section::VALUES);
    check(container_socket::GAME_NAME, container_socket::VALUES);
    check(enchantment::GAME_NAME, enchantment::VALUES);
    check(run_state::GAME_NAME, run_state::VALUES);
    check(card_tag::GAME_NAME, card_tag::VALUES);
    check(card_attribute::GAME_NAME, card_attribute::VALUES);
    check(player_attribute::GAME_NAME, player_attribute::VALUES);
}

#[test]
fn the_numbers_the_logic_compares_agree_with_the_names() {
    assert_eq!(card_type::name(known::CARD_TYPE_SKILL), Some("Skill"));
    assert_eq!(card_type::name(known::CARD_TYPE_ITEM), Some("Item"));
    assert_eq!(card_type::name(known::CARD_TYPE_ENCOUNTER_STEP), Some("EncounterStep"));
    assert_eq!(card_type::name(known::CARD_TYPE_SOCKET_EFFECT), Some("SocketEffect"));
    assert_eq!(inventory_section::name(known::SECTION_STASH), Some("Stash"));
    assert_eq!(inventory_section::name(known::SECTION_HAND), Some("Hand"));
    assert_eq!(combatant::name(known::COMBATANT_PLAYER), Some("Player"));
    assert_eq!(run_state::name(known::RUN_STATE_END_RUN_DEFEAT), Some("EndRunDefeat"));
    assert_eq!(run_state::name(known::RUN_STATE_END_RUN_VICTORY), Some("EndRunVictory"));
}

#[test]
fn an_instance_id_prefix_gives_the_card_kind() {
    assert_eq!(CardKind::from_instance_id("itm_EuUtMwz"), CardKind::Item);
    assert_eq!(CardKind::from_instance_id("enc_T4OLAka"), CardKind::Encounter);
    assert_eq!(CardKind::from_instance_id("ste_EzMYTc5"), CardKind::EncounterStep);
    assert_eq!(CardKind::from_instance_id("skl_tVCPvxR"), CardKind::Skill);
    assert_eq!(CardKind::from_instance_id("com_x"), CardKind::Combat);
    assert_eq!(CardKind::from_instance_id("pvp_9i8982w"), CardKind::Combat);
    assert_eq!(CardKind::from_instance_id("zzz_x"), CardKind::Unknown);
    assert_eq!(CardKind::from_instance_id("it"), CardKind::Unknown);
    assert_eq!(CardKind::Item.as_str(), "Item");
}
