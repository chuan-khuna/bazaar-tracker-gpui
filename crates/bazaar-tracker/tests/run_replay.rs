//! Whole recorded logs through the run state machine with scripted game memory and a clock that
//! follows the log's own timestamps.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use bazaar_tracker::log::{LogLine, parse_line};
use bazaar_tracker::memory::fake::FakeMemory;
use bazaar_tracker::memory::{BoardState, Item, PlayerStats, RunInit, Snapshot};
use bazaar_tracker::run::{Clock, Event, EventKind, ManualClock, Output, RunMachine};

const START_MS: i64 = 1_761_900_000_000;

/// The log's `[hh:mm:ss.fff]` as milliseconds into the day.
fn time_of_day_ms(line: &str) -> Option<i64> {
    let stamp = line.strip_prefix('[')?.get(..12)?;
    let (hours, rest) = stamp.split_once(':')?;
    let (minutes, rest) = rest.split_once(':')?;
    let (seconds, millis) = rest.split_once('.')?;
    Some(((hours.parse::<i64>().ok()? * 60 + minutes.parse::<i64>().ok()?) * 60 + seconds.parse::<i64>().ok()?) * 1000 + millis.parse::<i64>().ok()?)
}

fn read_log(name: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/logs").join(name);
    std::fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect()
}

fn item(id: &str) -> Item {
    Item { instance_id: id.to_owned(), template_id: Some(format!("tpl-{id}")), card_type: "Item".into(), tier: "Bronze".into(), size: "Small".into(), ..Item::default() }
}

/// Play `lines` through a machine whose memory always shows a board that knows every card the log
/// places, with `offered` on offer.
fn replay(lines: &[String], offered: &[&str]) -> Vec<Event> {
    let placed: BTreeSet<String> = lines
        .iter()
        .filter_map(|line| match parse_line(line) {
            Some(LogLine::CardPlaced { instance_id, .. }) => Some(instance_id),
            _ => None,
        })
        .collect();
    let memory = FakeMemory::new();
    memory.set_board(BoardState {
        player: Snapshot { stats: PlayerStats { hero: Some("Jules".into()), gold: Some(10), day: Some(1), hour: Some(0), victories: Some(0), losses: Some(0), ..PlayerStats::default() }, ..Snapshot::default() },
        selection_set: offered.iter().map(|id| (*id).to_owned()).collect(),
        entities: placed.iter().map(|id| item(id)).collect(),
        ..BoardState::default()
    });
    memory.set_run_init(RunInit { run_id: Some("3f2a9c1e-real-run".into()), play_mode: Some("Normal".into()), ..RunInit::default() });

    let clock = ManualClock::new(START_MS);
    let mut machine = RunMachine::new(Box::new(memory), Arc::new(clock.clone()));
    let first = lines.iter().find_map(|line| time_of_day_ms(line)).unwrap();
    let mut events = Vec::new();
    for line in lines {
        if let Some(at) = time_of_day_ms(line) {
            let target = START_MS + at - first;
            // Let the machine's timers fire at their own times on the way to the next line.
            while let Some(due) = machine.next_due_ms().filter(|due| *due <= target) {
                clock.set(due.max(clock.now_ms()));
                machine.tick();
                events.extend(machine.take_outputs());
            }
            clock.set(target);
        }
        machine.on_raw_line(line);
        events.extend(machine.take_outputs());
    }
    clock.advance(5000);
    machine.tick();
    events.extend(machine.take_outputs());
    events
        .into_iter()
        .filter_map(|output| match output {
            Output::Event(event) => Some(event),
            Output::Rebound { .. } => None,
        })
        .collect()
}

fn kinds(events: &[Event]) -> Vec<&'static str> {
    events.iter().map(|event| event.kind.name()).collect()
}

#[test]
fn a_recorded_run_start_gives_a_run_started_selections_and_a_reroll() {
    let lines = read_log("real-run-start.log");
    let events = replay(&lines, &["enc__lsiwsJ", "enc_x", "enc_y"]);

    assert_eq!(kinds(&events).first(), Some(&"RunStarted"));
    assert!(events.iter().all(|event| event.run_id == "3f2a9c1e-real-run"));
    assert!(events.windows(2).all(|pair| pair[0].timestamp_ms <= pair[1].timestamp_ms), "events are in time order");

    let selected: Vec<&str> = events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Selected { instance_id, .. } => Some(instance_id.as_str()),
            _ => None,
        })
        .collect();
    // Every placed card the log names was selected once, in log order, and so was the skill.
    let expected_placed: Vec<String> = lines
        .iter()
        .filter_map(|line| match parse_line(line) {
            Some(LogLine::CardPlaced { instance_id, .. }) => Some(instance_id),
            Some(LogLine::SkillSelected { skill_id, .. }) => Some(skill_id),
            _ => None,
        })
        .collect();
    assert_eq!(selected, expected_placed.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(selected.contains(&"skl_tVCPvxR"));

    assert_eq!(kinds(&events).iter().filter(|kind| **kind == "Rerolled").count(), 1);
    assert!(kinds(&events).contains(&"OptionsPresented"));
}

#[test]
fn the_first_options_come_after_the_first_deal_has_settled() {
    let lines = read_log("real-run-start.log");
    let events = replay(&lines, &["enc__lsiwsJ", "enc_x", "enc_y"]);
    let options = events.iter().find(|event| event.kind.name() == "OptionsPresented").unwrap();
    let EventKind::OptionsPresented { context, options: offered, .. } = &options.kind else { panic!() };
    assert_eq!(context, "ChoiceState");
    assert_eq!(offered.len(), 3);
    // ChoiceState was entered at 21:21:23.356, the log began at 21:21:18.233, and the settle takes 500 ms.
    assert_eq!(options.timestamp_ms, START_MS + (23_356 - 18_233) + 500);
}
