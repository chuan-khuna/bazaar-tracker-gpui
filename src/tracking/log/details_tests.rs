use bazaar_tracker::run::EventKind;

use super::*;
use crate::tracking::test_events::event;

fn details(running: bool) -> Details {
    Details::new(PathBuf::from("C:\\Game\\Player.log"), PathBuf::from("C:\\Data\\history.db"), running)
}

fn line(details: &Details, label: &str) -> String {
    details.lines().into_iter().find(|(name, _)| *name == label).unwrap().1
}

fn attach() -> Notice {
    Notice::Connection(ConnectionChange::Connected {
        pid: 11776,
        unity_version: "Unity 6 x64 (unverified)".into(),
        details: AttachDetails {
            process_name: "TheBazaar".into(),
            unity_version: "6000.3.11".into(),
            offsets: "Unity 6 x64 (unverified)".into(),
            offsets_verified: false,
            mono_module: "mono-2.0-bdwgc.dll".into(),
            mono_module_base: 0x7ffa_c853_0000,
            root_domain: 0x0218_1a73_2d20,
        },
    })
}

#[test]
fn before_the_game_is_found_the_block_has_the_paths_and_dashes() {
    let block = details(true);
    assert_eq!(line(&block, "State"), "Waiting for the game");
    assert_eq!(line(&block, "Process"), "TheBazaar");
    assert_eq!(line(&block, "Unity"), "—");
    assert_eq!(line(&block, "Player.log"), "C:\\Game\\Player.log");
    assert_eq!(line(&block, "history.db"), "C:\\Data\\history.db");
    assert_eq!(line(&block, "Last event"), "none yet");
}

#[test]
fn a_tracker_that_did_not_start_says_so() {
    assert_eq!(line(&details(false), "State"), "The tracker is not running");
}

#[test]
fn attaching_fills_in_what_attaching_found() {
    let mut block = details(true);
    block.apply(&attach());
    assert_eq!(line(&block, "State"), "Attached");
    assert_eq!(line(&block, "Process"), "TheBazaar (pid 11776)");
    assert_eq!(line(&block, "Unity"), "6000.3.11");
    assert_eq!(line(&block, "Offsets"), "Unity 6 x64 (unverified) · not verified");
    assert_eq!(line(&block, "Mono module"), "mono-2.0-bdwgc.dll at 0x7ffac8530000");
    assert_eq!(line(&block, "Root domain"), "0x2181a732d20");
}

#[test]
fn verified_offsets_say_so() {
    let mut block = details(true);
    block.apply(&Notice::Connection(ConnectionChange::Connected { pid: 1, unity_version: "x".into(), details: AttachDetails { offsets: "Unity 2022".into(), offsets_verified: true, ..AttachDetails::default() } }));
    assert_eq!(line(&block, "Offsets"), "Unity 2022 · verified");
    assert_eq!(line(&block, "Mono module"), "— at —");
}

#[test]
fn losing_the_game_goes_back_to_waiting() {
    let mut block = details(true);
    block.apply(&attach());
    block.apply(&Notice::Connection(ConnectionChange::Disconnected { reason: "gone".into() }));
    assert_eq!(line(&block, "State"), "Waiting for the game");
    assert_eq!(line(&block, "Unity"), "—");
}

#[test]
fn a_saved_event_sets_the_time_of_the_last_one() {
    let mut block = details(true);
    block.apply(&Notice::EventSaved(event("e", 1_793_394_840_000, EventKind::Rerolled)));
    let shown = line(&block, "Last event");
    assert_eq!(shown.len(), 8, "{shown}");
    assert_eq!(shown.matches(':').count(), 2);
    block.apply(&Notice::LogError("ignored".into()));
    assert_eq!(line(&block, "Last event"), shown);
}
