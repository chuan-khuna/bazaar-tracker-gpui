//! Options offered, items and skills selected, items sold and rerolls.

use super::event::{EventKind, Outcome, PresentedOption};
use super::machine::{DEALT_PREVIOUS_CAP, Emit, RunMachine, is_combat_state, is_option_state};
use crate::enums::CardKind;
use crate::memory::BoardState;

/// A choice the player made, waiting to be built into a Selected Event.
#[derive(Clone, Debug)]
pub struct Selection {
    pub(super) run_id: String,
    pub(super) instance_id: String,
    pub(super) template_id: Option<String>,
    pub(super) section: Option<String>,
    pub(super) target: Option<String>,
    pub(super) merged_into: Option<String>,
}

impl RunMachine {
    pub(super) fn handle_state_changed(&mut self, to: &str) {
        let previous = self.app_state.clone();
        if previous.as_deref().is_some_and(is_combat_state) && !is_combat_state(to) {
            self.queue_combat_end(Self::outcome_for_state(to));
        }
        match to {
            "EndRunVictoryState" => self.end_run(Outcome::Win),
            "EndRunDefeatState" => self.end_run(Outcome::Loss),
            _ => {}
        }
        self.app_state = Some(to.to_owned());
        if previous.as_deref() == Some(to) {
            if is_option_state(to) {
                self.schedule_settle();
            }
            return;
        }
        self.retire_current_options();
        if is_option_state(to) {
            self.schedule_settle();
            return;
        }
        // Leaving the option states: a settle that is still waiting no longer applies.
        self.settle_generation += 1;
        if is_combat_state(to) {
            self.enter_combat(to);
        }
    }

    pub(super) fn handle_game_sim_summary(&mut self, dealt: u32, spawned: u32) {
        if dealt > 0 {
            self.options_dirty = true;
            if self.app_state.as_deref().is_some_and(is_option_state) {
                self.schedule_settle();
            }
        }
        if spawned > 0 {
            self.on_cards_spawned();
        }
    }

    /// The options on screen are no longer current: remember them as recent.
    fn retire_current_options(&mut self) {
        let current = std::mem::take(&mut self.dealt_current);
        self.remember_previous(current);
    }

    fn remember_previous(&mut self, ids: Vec<String>) {
        for id in ids {
            if !self.dealt_previous.contains(&id) {
                self.dealt_previous.push(id);
            }
        }
        if self.dealt_previous.len() > DEALT_PREVIOUS_CAP {
            let excess = self.dealt_previous.len() - DEALT_PREVIOUS_CAP;
            self.dealt_previous.drain(..excess);
        }
    }

    /// Take the options on offer from a board read, retiring the ones that are gone.
    fn refresh_options_from_board(&mut self, board: &BoardState) {
        let gone: Vec<String> = self.dealt_current.iter().filter(|id| !board.selection_set.contains(id)).cloned().collect();
        self.remember_previous(gone);
        self.dealt_current = board.selection_set.clone();
    }

    /// Build an OptionsPresented Event from the options not yet reported. In settle mode it does
    /// so only if the game dealt cards since the last report; otherwise (a choice is about to be
    /// reported and the options must come first) it reports `presented`.
    pub(super) fn build_options(&mut self, settle: bool, presented: Option<Vec<String>>, context: &str) {
        let Some(run_id) = self.run_id.clone() else { return };
        if settle {
            if !self.options_dirty {
                return;
            }
            self.options_dirty = false;
        }
        let Some(board) = self.read_board() else {
            if settle {
                self.options_dirty = true;
            }
            return;
        };
        let ids = if settle {
            self.refresh_options_from_board(&board);
            self.dealt_current.clone()
        } else {
            presented.unwrap_or_default()
        };
        let ids: Vec<String> = ids.into_iter().filter(|id| !self.reported_option_ids.contains(id)).collect();
        if ids.is_empty() {
            return;
        }
        let options = ids.iter().map(|id| self.presented_option(id)).collect();
        let kind = EventKind::OptionsPresented { context: context.to_owned(), options, snapshot: board.player.clone() };
        self.emit(&run_id, board.player.stats.day, board.player.stats.hour, kind);
        if let Some(event_id) = self.last_event_id() {
            for id in &ids {
                self.option_event_of.insert(id.clone(), event_id.clone());
            }
        }
        self.reported_option_ids.extend(ids);
        self.options_dirty = false;
    }

    fn presented_option(&self, id: &str) -> PresentedOption {
        let entity = self.entity_index.get(id);
        PresentedOption {
            instance_id: id.to_owned(),
            kind: CardKind::from_instance_id(id).as_str().to_owned(),
            size: entity.map_or_else(|| "Unknown".to_owned(), |item| item.size.clone()),
            template_id: entity.and_then(|item| item.template_id.clone()),
            tier: entity.map(|item| item.tier.clone()),
            enchantment: entity.and_then(|item| item.enchantment.clone()),
        }
    }

    /// The log says a card was placed ("Card Purchased"): the player took an offered card, or
    /// bought a duplicate that merged into one they had.
    pub(super) fn handle_placed(&mut self, instance_id: &str, template_id: &str, target: &str, section: &str) {
        let in_dealt = self.dealt_current.iter().any(|id| id == instance_id);
        let in_previous = self.dealt_previous.iter().any(|id| id == instance_id);
        let known_to_player = self.last_player_item_ids.iter().any(|id| id == instance_id);
        if !self.selected_ids.contains(instance_id) && (in_dealt || in_previous || !known_to_player) {
            self.selected_ids.insert(instance_id.to_owned());
            if instance_id.starts_with("pvp_") {
                self.combat.is_pvp = true;
            }
            if in_dealt {
                let context = self.app_state.clone().unwrap_or_default();
                self.queue.push_back(Emit::Options { settle: false, presented: Some(self.dealt_current.clone()), context });
            }
            self.ensure_run();
            self.queue_selected(instance_id, Some(template_id.to_owned()), Some(section.to_owned()), Some(target.to_owned()), None);
        } else if self.app_state.as_deref().is_some_and(is_option_state) && CardKind::from_instance_id(instance_id) == CardKind::Item {
            // Buying a copy of an item the player has upgrades it: the offered copy disappears.
            let candidates: Vec<String> =
                self.dealt_current.iter().filter(|id| !self.selected_ids.contains(*id) && CardKind::from_instance_id(id) == CardKind::Item).cloned().collect();
            if candidates.is_empty() {
                return;
            }
            self.ensure_run();
            if let Some(run_id) = self.run_id.clone() {
                self.queue.push_back(Emit::Merge { run_id, candidates, merged_into: instance_id.to_owned() });
            }
        }
    }

    pub(super) fn handle_skill_selected(&mut self, skill_id: &str, socket: &str) {
        if !self.selected_ids.insert(skill_id.to_owned()) {
            return;
        }
        let context = self.app_state.clone().unwrap_or_default();
        self.queue.push_back(Emit::Options { settle: false, presented: Some(self.dealt_current.clone()), context });
        self.ensure_run();
        self.queue_selected(skill_id, None, None, Some(socket.to_owned()), None);
    }

    fn queue_selected(&mut self, instance_id: &str, template_id: Option<String>, section: Option<String>, target: Option<String>, merged_into: Option<String>) {
        let Some(run_id) = self.run_id.clone() else { return };
        let selection = Selection { run_id, instance_id: instance_id.to_owned(), template_id, section, target, merged_into };
        self.queue.push_back(Emit::Selected(Box::new(selection)));
    }

    pub(super) fn build_selected(&mut self, selection: Selection) {
        let board = self.read_board();
        let entity = self.entity_index.get(&selection.instance_id).cloned();
        let snapshot = board.as_ref().map(|board| board.player.clone()).unwrap_or_default();
        let kind = EventKind::Selected {
            card_kind: CardKind::from_instance_id(&selection.instance_id).as_str().to_owned(),
            options_event_id: self.option_event_of.get(&selection.instance_id).cloned(),
            template_id: selection.template_id.or_else(|| entity.as_ref().and_then(|item| item.template_id.clone())),
            tier: entity.as_ref().map(|item| item.tier.clone()),
            enchantment: entity.as_ref().and_then(|item| item.enchantment.clone()),
            section: selection.section,
            target: selection.target,
            merged_into: selection.merged_into,
            instance_id: selection.instance_id,
            snapshot,
        };
        self.emit(&selection.run_id, board.as_ref().and_then(|b| b.player.stats.day), board.as_ref().and_then(|b| b.player.stats.hour), kind);
    }

    /// After a duplicate was bought: if exactly one of the offered copies is gone from the game,
    /// that is the one that merged.
    pub(super) fn build_merge(&mut self, run_id: &str, candidates: &[String], merged_into: &str) {
        let Some(board) = self.read_board() else { return };
        let gone: Vec<&String> = candidates.iter().filter(|id| board.entity(id).is_none()).collect();
        let [only] = gone.as_slice() else { return };
        let only = (*only).clone();
        self.selected_ids.insert(only.clone());
        self.build_selected(Selection { run_id: run_id.to_owned(), instance_id: only, template_id: None, section: None, target: None, merged_into: Some(merged_into.to_owned()) });
    }

    pub(super) fn handle_command(&mut self, command: &str, succeeded: bool) {
        if !succeeded {
            return;
        }
        if command == "AbandonRunCommand" {
            self.end_run(Outcome::Loss);
        } else if command.to_ascii_lowercase().contains("reroll") {
            self.ensure_run();
            if let Some(run_id) = self.run_id.clone() {
                self.queue.push_back(Emit::Rerolled { run_id });
            }
        } else if command.to_ascii_lowercase().contains("sellcard") {
            self.ensure_run();
            if let Some(run_id) = self.run_id.clone() {
                let before = self.last_player_item_ids.clone();
                self.queue.push_back(Emit::Sold { run_id, before });
            }
        }
    }

    /// After a sale: the player's items that are no longer there were sold.
    pub(super) fn build_sold(&mut self, run_id: &str, before: &[String]) {
        let Some(board) = self.read_board() else { return };
        let after = board.player_item_ids();
        let gone: Vec<String> = before.iter().filter(|id| !after.contains(id) && !self.sold_ids.contains(*id)).cloned().collect();
        for id in gone {
            self.sold_ids.insert(id.clone());
            let entity = self.entity_index.get(&id).cloned();
            let kind = EventKind::Sold {
                template_id: entity.as_ref().and_then(|item| item.template_id.clone()),
                tier: entity.as_ref().map(|item| item.tier.clone()),
                enchantment: entity.as_ref().and_then(|item| item.enchantment.clone()),
                instance_id: id,
                snapshot: board.player.clone(),
            };
            self.emit(run_id, board.player.stats.day, board.player.stats.hour, kind);
        }
    }
}
