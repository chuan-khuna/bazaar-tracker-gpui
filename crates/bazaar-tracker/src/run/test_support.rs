//! Shared by the tests of the run state machine: a machine with a fake memory and a clock the test
//! moves, and builders for boards.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::machine::{Output, RunMachine};
use super::{Clock, Event, ManualClock};
use crate::memory::fake::FakeMemory;
use crate::memory::{BoardState, Item, PlayerStats, RunInit, Snapshot};

pub(super) const START_MS: i64 = 1_000_000;

/// A machine with a fake memory and a clock the test moves. Ids are `e1`, `e2`, ... and `local-`
/// runs are `local-e<n>`.
pub(super) struct Harness {
    pub(super) machine: RunMachine,
    pub(super) memory: FakeMemory,
    pub(super) clock: ManualClock,
}

pub(super) fn harness() -> Harness {
    let memory = FakeMemory::new();
    let clock = ManualClock::new(START_MS);
    let counter = Arc::new(AtomicU32::new(0));
    let machine = RunMachine::with_ids(Box::new(memory.clone()), Arc::new(clock.clone()), Box::new(move || format!("e{}", counter.fetch_add(1, Ordering::SeqCst) + 1)));
    Harness { machine, memory, clock }
}

impl Harness {
    pub(super) fn raw(&mut self, line: &str) {
        self.machine.on_raw_line(&format!("[00:00:00.000] {line}"));
    }

    pub(super) fn state(&mut self, from: &str, to: &str) {
        self.raw(&format!("[AppState] State changed from [{from}] to [{to}]"));
    }

    pub(super) fn dealt(&mut self, dealt: u32) {
        self.raw(&format!("[GameSimHandler] GameSim cards: id=abc, dealt={dealt}, spawned=0, disposed=0"));
    }

    pub(super) fn placed(&mut self, id: &str, target: &str, section: &str) {
        self.raw(&format!("[BoardManager] Card Purchased: InstanceId: {id} - TemplateIdabc-123 - Target:{target} - Section{section}"));
    }

    pub(super) fn command(&mut self, command: &str) {
        self.raw(&format!("[HttpGameClient] Command completed: type={command}, requestId=1, status=200 OK, durationMs=5, result=success, endpoint=/commands"));
    }

    pub(super) fn run_initialized(&mut self) {
        self.raw("[GameMessageHandler] Handled: type=NetMessageRunInitialized, id=Ydf, durationMs=0, result=success");
    }

    /// Let `ms` pass, stopping at each moment the machine has something due, as a service ticking
    /// often would.
    pub(super) fn advance(&mut self, ms: i64) {
        let target = self.clock.now_ms() + ms;
        while let Some(due) = self.machine.next_due_ms().filter(|due| *due <= target) {
            self.clock.set(due.max(self.clock.now_ms()));
            self.machine.tick();
        }
        self.clock.set(target);
        self.machine.tick();
    }

    pub(super) fn outputs(&mut self) -> Vec<Output> {
        self.machine.take_outputs()
    }

    pub(super) fn events(&mut self) -> Vec<Event> {
        self.outputs()
            .into_iter()
            .filter_map(|output| match output {
                Output::Event(event) => Some(event),
                Output::Rebound { .. } => None,
            })
            .collect()
    }

    pub(super) fn start_run(&mut self, run_id: &str) {
        self.memory.set_run_init(RunInit { run_id: Some(run_id.to_owned()), play_mode: Some("Normal".into()), build_id: Some("b1".into()), environment: Some("prod".into()) });
        self.run_initialized();
    }
}

pub(super) fn names(events: &[Event]) -> Vec<&'static str> {
    events.iter().map(|event| event.kind.name()).collect()
}

pub(super) fn item(id: &str, tier: &str, size: &str) -> Item {
    Item { instance_id: id.to_owned(), template_id: Some(format!("tpl-{id}")), card_type: "Item".into(), tier: tier.into(), size: size.into(), ..Item::default() }
}

pub(super) fn stats(gold: i32) -> PlayerStats {
    PlayerStats { hero: Some("Jules".into()), gold: Some(gold), day: Some(2), hour: Some(3), victories: Some(1), losses: Some(0), health: Some(300), health_max: Some(300), ..PlayerStats::default() }
}

/// A board where the player owns `owned`, `offered` is on offer, and everything is known.
pub(super) fn board(owned: &[Item], offered: &[Item], gold: i32) -> BoardState {
    let mut entities: Vec<Item> = owned.to_vec();
    entities.extend(offered.iter().cloned());
    BoardState {
        player: Snapshot { board: owned.to_vec(), stats: stats(gold), ..Snapshot::default() },
        selection_set: offered.iter().map(|item| item.instance_id.clone()).collect(),
        entities,
        run_state: Some("Choice".into()),
        ..BoardState::default()
    }
}

