use super::*;

fn state(from: &str, to: &str) -> Option<LogLine> {
    Some(LogLine::StateChanged { from: from.into(), to: to.into() })
}

#[test]
fn the_timestamp_prefix_is_stripped() {
    assert_eq!(strip_timestamp("[21:21:23.356] [AppState] hello  "), "[AppState] hello");
    assert_eq!(strip_timestamp("[21:21:23.356]   x"), "x");
    // Not a timestamp: left alone.
    assert_eq!(strip_timestamp("[AppState] x"), "[AppState] x");
    assert_eq!(strip_timestamp("[1:2:3.4] x"), "[1:2:3.4] x");
}

#[test]
fn state_changes_are_parsed() {
    assert_eq!(parse_line("[21:21:23.356] [AppState] State changed from [StartRunAppState] to [ChoiceState]"), state("StartRunAppState", "ChoiceState"));
    assert_eq!(parse_line("[21:21:23.350] [AppState] State changed from [null] to [StartRunAppState]"), state("null", "StartRunAppState"));
    assert_eq!(parse_line("[00:00:00.000] [AppState] State changed from [ChoiceState] to [PVPCombatState]"), state("ChoiceState", "PVPCombatState"));
}

#[test]
fn near_misses_of_state_changes_are_not_parsed() {
    // No brackets around the states.
    assert_eq!(parse_line("[AppState] State changed from A to B"), None);
    // A state name with a character outside the pattern.
    assert_eq!(parse_line("[AppState] State changed from [A-B] to [C]"), None);
    // The words are there but not the shape.
    assert_eq!(parse_line("[AppState] State changed to [ChoiceState]"), None);
    assert_eq!(parse_line("Input System module state changed to: Initialized."), None);
}

#[test]
fn skill_selections_are_parsed() {
    assert_eq!(
        parse_line("[21:21:42.841] [AppState] Selected skill skl_tVCPvxR to socket SkillSocket_0"),
        Some(LogLine::SkillSelected { skill_id: "skl_tVCPvxR".into(), socket: "SkillSocket_0".into() })
    );
    // Ids may contain a dash.
    assert_eq!(
        parse_line("[AppState] Selected skill skl_a-b to socket SkillSocket_1"),
        Some(LogLine::SkillSelected { skill_id: "skl_a-b".into(), socket: "SkillSocket_1".into() })
    );
}

#[test]
fn near_misses_of_skill_selections_are_not_parsed() {
    assert_eq!(parse_line("[AppState] Selected skill skl_x"), None);
    assert_eq!(parse_line("[AppState] Selected item itm_x to socket Socket_0"), None);
    // The substring guard wants the closing bracket of the category before the words.
    assert_eq!(parse_line("Selected skill skl_x to socket S_0"), None);
}

#[test]
fn game_sim_summaries_are_parsed() {
    assert_eq!(
        parse_line("[21:21:33.589] [GameSimHandler] GameSim cards: id=dqT, dealt=3, spawned=0, disposed=0"),
        Some(LogLine::GameSimSummary { dealt: 3, spawned: 0, disposed: 0 })
    );
    assert_eq!(
        parse_line("[GameSimHandler] GameSim cards: id=, dealt=0, spawned=11, disposed=2"),
        Some(LogLine::GameSimSummary { dealt: 0, spawned: 11, disposed: 2 })
    );
}

#[test]
fn near_misses_of_game_sim_summaries_are_not_parsed() {
    assert_eq!(parse_line("[GameSimHandler] GameSim cards: id=dqT, dealt=3, spawned=0"), None);
    assert_eq!(parse_line("[GameSimHandler] GameSim cards: id=dqT, dealt=x, spawned=0, disposed=0"), None);
    // Too large for a count.
    assert_eq!(parse_line("[GameSimHandler] GameSim cards: id=dqT, dealt=99999999999, spawned=0, disposed=0"), None);
}

#[test]
fn placed_cards_are_parsed() {
    assert_eq!(
        parse_line("[21:22:59.034] [BoardManager] Card Purchased: InstanceId: itm_I_dRbS- - TemplateIdda766a09-0352-4966-829f-20bda8da48d7 - Target:PlayerSocket_3 - SectionPlayer"),
        Some(LogLine::CardPlaced {
            instance_id: "itm_I_dRbS-".into(),
            template_id: "da766a09-0352-4966-829f-20bda8da48d7".into(),
            target: "PlayerSocket_3".into(),
            section: "Player".into(),
        })
    );
    assert_eq!(
        parse_line("[BoardManager] Card Purchased: InstanceId: itm_zl-t1UD - TemplateId825700a8-f21b-4867-ad08-d752f13a51ab - Target:PlayerStorageSocket_0 - SectionStorage"),
        Some(LogLine::CardPlaced {
            instance_id: "itm_zl-t1UD".into(),
            template_id: "825700a8-f21b-4867-ad08-d752f13a51ab".into(),
            target: "PlayerStorageSocket_0".into(),
            section: "Storage".into(),
        })
    );
}

#[test]
fn near_misses_of_placed_cards_are_not_parsed() {
    // A template id that is not hex.
    assert_eq!(parse_line("[BoardManager] Card Purchased: InstanceId: itm_x - TemplateIdzzzz - Target:S_0 - SectionPlayer"), None);
    // Missing the section.
    assert_eq!(parse_line("[BoardManager] Card Purchased: InstanceId: itm_x - TemplateIdabc - Target:S_0"), None);
    // Another BoardManager line.
    assert_eq!(parse_line("[21:24:30.160] [BoardManager] Sold Card itm_EuUtMwz for 1 gold."), None);
}

#[test]
fn completed_commands_are_parsed() {
    assert_eq!(
        parse_line("[21:21:33.589] [NetworkManager] [HttpGameClient] Command completed: type=SelectEncounterCommand, requestId=2, status=200 OK, durationMs=57, result=success, endpoint=/commands"),
        Some(LogLine::CommandCompleted { command: "SelectEncounterCommand".into(), succeeded: true })
    );
    assert_eq!(
        parse_line("[HttpGameClient] Command completed: type=SellCardCommand, requestId=9, status=400, result=failure, endpoint=/commands"),
        Some(LogLine::CommandCompleted { command: "SellCardCommand".into(), succeeded: false })
    );
}

#[test]
fn near_misses_of_completed_commands_are_not_parsed() {
    // The type does not end in "Command".
    assert_eq!(parse_line("[HttpGameClient] Command completed: type=Ping, result=success"), None);
    // No result.
    assert_eq!(parse_line("[HttpGameClient] Command completed: type=SellCardCommand, requestId=9"), None);
    assert_eq!(parse_line("[HttpGameClient] Command started: type=SellCardCommand, result=success"), None);
}

#[test]
fn run_initialization_is_recognised_by_its_substring_alone() {
    assert_eq!(parse_line("[21:21:20.558] [GameMessageHandler] Handled: type=NetMessageRunInitialized, id=Ydf, durationMs=0, result=success"), Some(LogLine::RunInitialized));
    // "Received" is the line before it, and is not the trigger.
    assert_eq!(parse_line("[21:21:20.557] [NetMessageProcessor] Received: type=NetMessageRunInitialized, id=Ydf"), None);
    assert_eq!(parse_line("[GameMessageHandler] Handled: type=NetMessageGameStateSync, id=m2z"), None);
}

#[test]
fn combat_preparation_and_completion_are_parsed() {
    assert_eq!(parse_line("[21:23:35.691] [CombatSimHandler] Combat prepared: frames=659, effectsMs=5"), Some(LogLine::CombatPrepared { frames: Some(659) }));
    assert_eq!(parse_line("[CombatSimHandler] Combat prepared: frames=many"), Some(LogLine::CombatPrepared { frames: None }));
    assert_eq!(parse_line("[21:23:48.042] [CombatSimHandler] Combat simulation completed: durationMs=12356"), Some(LogLine::CombatCompleted));
}

#[test]
fn near_misses_of_combat_lines_are_not_parsed() {
    // Right words, wrong category.
    assert_eq!(parse_line("[Other] Combat prepared: frames=659"), None);
    assert_eq!(parse_line("[CombatSimHandler] Combat simulation started"), None);
}

#[test]
fn ordinary_lines_are_not_parsed() {
    for line in ["", "   ", "[21:21:23.282] [BoardManager] BoardManager initialized.", "BoxCollider does not support negative scale or size.", "- Loaded All Assemblies, in  7.288 seconds"] {
        assert_eq!(parse_line(line), None, "{line:?}");
    }
}

/// Parse every line of a recorded log.
fn parse_fixture(name: &str) -> Vec<LogLine> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/logs").join(name);
    std::fs::read_to_string(path).unwrap().lines().filter_map(parse_line).collect()
}

#[test]
fn a_real_log_excerpt_parses_into_the_expected_lines() {
    let lines = parse_fixture("real-run-start.log");
    assert_eq!(lines.iter().filter(|line| matches!(line, LogLine::RunInitialized)).count(), 1);
    assert!(lines.contains(&LogLine::CombatPrepared { frames: Some(659) }));
    assert!(lines.contains(&LogLine::CombatCompleted));
    assert!(lines.contains(&LogLine::SkillSelected { skill_id: "skl_tVCPvxR".into(), socket: "SkillSocket_0".into() }));
    let states: Vec<_> = lines
        .iter()
        .filter_map(|line| match line {
            LogLine::StateChanged { to, .. } => Some(to.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(states.first(), Some(&"StartRunAppState"));
    assert!(states.contains(&"CombatState") && states.contains(&"ReplayState") && states.contains(&"LootState"));
    assert!(lines.iter().any(|line| matches!(line, LogLine::CardPlaced { section, .. } if section == "Player")));
    assert!(lines.iter().any(|line| matches!(line, LogLine::CommandCompleted { command, succeeded: true } if command == "SelectItemCommand")));
}

#[test]
fn a_real_pvp_excerpt_parses_into_a_pvp_combat() {
    let lines = parse_fixture("real-pvp-combat.log");
    assert!(lines.contains(&state("ChoiceState", "PVPCombatState").unwrap()));
    assert!(lines.contains(&LogLine::CombatPrepared { frames: Some(415) }));
    assert!(lines.iter().any(|line| matches!(line, LogLine::CardPlaced { instance_id, .. } if instance_id.starts_with("pvp_"))));
}
