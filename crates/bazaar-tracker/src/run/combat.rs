//! Combats: the two boards just before a fight, and how it ended. Built out in ticket 13; until
//! then the machine only notes whether a fight is a PvP one.

use super::event::Outcome;
use super::machine::RunMachine;

/// What is known about the combat in progress.
#[derive(Debug, Default)]
pub(super) struct CombatState {
    pub is_pvp: bool,
}

#[derive(Clone, Debug)]
pub struct PendingCombatStart;

#[derive(Clone, Debug)]
pub struct PendingCombatEnd;

impl RunMachine {
    pub(super) fn enter_combat(&mut self, _state: &str) {}
    pub(super) fn on_cards_spawned(&mut self) {}
    pub(super) fn handle_combat_prepared(&mut self, _frames: Option<u32>) {}
    pub(super) fn queue_combat_end(&mut self, _known: Option<Outcome>) {}
    pub(super) fn on_combat_capture_fired(&mut self, _generation: u64) {}
    pub(super) fn on_combat_end_retry(&mut self, _pending: PendingCombatEnd) {}
    pub(super) fn build_combat_start(&mut self, _pending: PendingCombatStart) {}
    pub(super) fn build_combat_end(&mut self, _pending: PendingCombatEnd) {}
}
