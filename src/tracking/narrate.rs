//! Turns what the tracker reports into the plain sentences of the log panel. Nothing here prints a
//! Debug struct or JSON; cards are named by the catalog where it knows them.
//!
//! An Event is narrated alone, but some sentences need what came before it: whether a selection was
//! made from a shop, what a sold item was, and who a combat was against. The [`Narrator`] keeps
//! just enough of that.
//!
//! The live board is read every second or so; it is logged only when what the game offers changes
//! (a shop's items, an encounter choice), as a `now` row.

use std::collections::HashMap;

use bazaar_tracker::memory::{BoardState, ConnectionChange, Item, Snapshot};
use bazaar_tracker::run::{Event, EventKind, Outcome};
use bazaar_tracker::tracker::Notice;

use super::log::{Kind, LogRow};
use super::names::Names;

/// The most remembered Events of each sort, so that a long session does not grow without end.
const REMEMBERED: usize = 200;

/// Who a combat was against.
struct Combat {
    pvp: bool,
    opponent: Option<String>,
}

#[derive(Default)]
pub struct Narrator {
    /// The state each Options Event was offered in, by Event id (`Encounter` is a shop).
    contexts: HashMap<String, String>,
    combats: HashMap<String, Combat>,
    /// Everything the player had at the latest Snapshot, to look a sold item up in.
    items: Vec<Item>,
    /// The game state and the ids on offer at the last `now` row, so the same offer is logged once.
    offer: Option<(String, Vec<String>)>,
}

impl Narrator {
    /// The log row for a notice, if it is one worth a row. `now_ms` stamps the notices that carry no
    /// time of their own.
    pub fn row(&mut self, notice: &Notice, names: &Names, now_ms: i64) -> Option<LogRow> {
        let (kind, text, time_ms) = match notice {
            Notice::Connection(ConnectionChange::Connected { pid, unity_version, details }) => {
                let unity = if details.unity_version.is_empty() { unity_version.as_str() } else { details.unity_version.as_str() };
                (Kind::Info, format!("Connected to The Bazaar (pid {pid}, Unity {unity})"), now_ms)
            }
            Notice::Connection(ConnectionChange::Disconnected { reason }) => {
                self.offer = None;
                (Kind::Warn, format!("Lost the game: {reason}"), now_ms)
            }
            Notice::EventSaved(event) => (Kind::Event, self.event(event, names), event.timestamp_ms),
            Notice::RunRebound { .. } => (Kind::Info, "The run now has its real id; its events were moved to it".to_owned(), now_ms),
            Notice::SaveFailed { what, error, retry_in } => (Kind::Error, format!("Could not save {what}: {error}; trying again in {}s", retry_in.as_secs().max(1)), now_ms),
            Notice::LogError(error) => (Kind::Error, format!("Could not read Player.log: {error}"), now_ms),
            Notice::LiveSnapshot(board) => (Kind::Now, self.offer(board.as_ref(), names)?, now_ms),
        };
        Some(LogRow { time_ms, kind, text })
    }

    fn event(&mut self, event: &Event, names: &Names) -> String {
        match &event.kind {
            EventKind::RunStarted { hero, play_mode, .. } => {
                let parts: Vec<&str> = [hero.as_deref(), play_mode.as_deref()].into_iter().flatten().collect();
                if parts.is_empty() { "Run started".to_owned() } else { format!("Run started: {}", parts.join(", ")) }
            }
            EventKind::OptionsPresented { context, options, snapshot } => {
                self.remember_context(&event.id, context);
                let offered: Vec<String> = options.iter().map(|option| names.card(option.template_id.as_deref(), &option.instance_id)).collect();
                let count = if offered.len() == 1 { "1 option".to_owned() } else { format!("{} options", offered.len()) };
                let list = match offered.len() {
                    0 => String::new(),
                    1..=4 => format!(": {}", offered.join(", ")),
                    more => format!(": {} and {} more", offered[..4].join(", "), more - 4),
                };
                self.remember_items(snapshot);
                format!("Offered {count} ({}){list}", context_name(context))
            }
            EventKind::Selected { card_kind, instance_id, template_id, tier, snapshot, options_event_id, .. } => {
                let name = names.card(template_id.as_deref(), instance_id);
                let held = held(snapshot, instance_id);
                let description = describe(&name, tier.as_deref(), held.map(|item| item.size.as_str()));
                let shop = options_event_id.as_ref().and_then(|id| self.contexts.get(id)).is_some_and(|context| context == "EncounterState");
                let price = held.and_then(|item| item.attributes.get("BuyPrice")).copied().filter(|price| *price > 0);
                let text = match (card_kind.as_str(), price) {
                    ("Item", Some(price)) if shop => format!("Bought {description} for {price} gold"),
                    ("Item", _) => format!("Took {description}"),
                    ("Skill", _) => format!("Learned the skill {description}"),
                    (other, _) => format!("Chose {} {name}", other.to_lowercase()),
                };
                self.remember_items(snapshot);
                text
            }
            EventKind::Sold { instance_id, template_id, tier, snapshot, .. } => {
                let name = names.card(template_id.as_deref(), instance_id);
                let before = self.items.iter().find(|item| item.instance_id == *instance_id);
                let price = before.and_then(|item| item.attributes.get("SellPrice")).copied().filter(|price| *price > 0);
                let description = describe(&name, tier.as_deref(), before.map(|item| item.size.as_str()));
                self.remember_items(snapshot);
                match price {
                    Some(price) => format!("Sold {description} for {price} gold"),
                    None => format!("Sold {description}"),
                }
            }
            EventKind::Rerolled => "Refreshed the offers".to_owned(),
            EventKind::CombatStart { player, is_pvp, opponent_name, opponent_hero, .. } => {
                let opponent = opponent_name.clone().or_else(|| opponent_hero.clone());
                let text = match (is_pvp, &opponent) {
                    (true, Some(opponent)) => format!("Combat started: PvP against {opponent}"),
                    (true, None) => "Combat started: PvP".to_owned(),
                    (false, _) => "Combat started: PvE".to_owned(),
                };
                if self.combats.len() >= REMEMBERED {
                    self.combats.clear();
                }
                self.combats.insert(event.id.clone(), Combat { pvp: *is_pvp, opponent });
                self.remember_items(player);
                text
            }
            EventKind::CombatEnd { outcome, combat_start_event_id, .. } => {
                let result = match outcome {
                    Outcome::Win => "won",
                    Outcome::Loss => "lost",
                    Outcome::Unknown => "the result was not recorded",
                };
                let combat = combat_start_event_id.as_ref().and_then(|id| self.combats.get(id));
                match (outcome, combat) {
                    (Outcome::Unknown, _) => format!("Combat ended: {result}"),
                    (_, Some(Combat { pvp: true, opponent: Some(opponent) })) => format!("Combat ended: {result} against {opponent} (PvP)"),
                    (_, Some(Combat { pvp: true, opponent: None })) => format!("Combat ended: {result} (PvP)"),
                    (_, Some(Combat { pvp: false, .. })) => format!("Combat ended: {result} (PvE)"),
                    (_, None) => format!("Combat ended: {result}"),
                }
            }
            EventKind::RunEnded { outcome, victories, losses, .. } => {
                let word = match outcome {
                    Outcome::Win => "victory",
                    Outcome::Loss => "defeat",
                    Outcome::Unknown => "no result recorded",
                };
                match (victories, losses) {
                    (Some(victories), Some(losses)) => format!("Run ended: {word} ({}, {})", count(*victories, "win", "wins"), count(*losses, "loss", "losses")),
                    _ => format!("Run ended: {word}"),
                }
            }
        }
    }

    /// A sentence for what the game offers now, if it is a shop or an encounter choice and differs
    /// from the last one logged.
    fn offer(&mut self, board: Option<&BoardState>, names: &Names) -> Option<String> {
        let (state, title) = match board.and_then(|board| board.run_state.as_deref()) {
            Some(state @ "Encounter") => (state, "Shop"),
            Some(state @ "Choice") => (state, "Encounter choice"),
            Some(state @ "Loot") => (state, "Loot"),
            _ => {
                self.offer = None;
                return None;
            }
        };
        let board = board?;
        if board.selection_set.is_empty() {
            self.offer = None;
            return None;
        }
        let offer = (state.to_owned(), board.selection_set.clone());
        if self.offer.as_ref() == Some(&offer) {
            return None;
        }
        self.offer = Some(offer);
        let cards: Vec<String> = board
            .selection_set
            .iter()
            .map(|id| match board.entity(id) {
                Some(entity) if id.starts_with("itm_") => describe(&names.card(entity.template_id.as_deref(), id), Some(entity.tier.as_str()), Some(entity.size.as_str())),
                Some(entity) => names.card(entity.template_id.as_deref(), id),
                None => id.clone(),
            })
            .collect();
        Some(format!("{title}: {}", cards.join(", ")))
    }

    fn remember_context(&mut self, event_id: &str, context: &str) {
        if self.contexts.len() >= REMEMBERED {
            self.contexts.clear();
        }
        self.contexts.insert(event_id.to_owned(), context.to_owned());
    }

    fn remember_items(&mut self, snapshot: &Snapshot) {
        self.items = snapshot.board.iter().chain(&snapshot.stash).chain(&snapshot.skills).cloned().collect();
    }
}

/// The item with this id in the Snapshot, wherever the player has it.
fn held<'a>(snapshot: &'a Snapshot, instance_id: &str) -> Option<&'a Item> {
    snapshot.board.iter().chain(&snapshot.stash).chain(&snapshot.skills).find(|item| item.instance_id == instance_id)
}

/// `Pasta Maker (Bronze, Medium)`; the brackets hold what is known.
fn describe(name: &str, tier: Option<&str>, size: Option<&str>) -> String {
    let details: Vec<&str> = [tier, size].into_iter().flatten().filter(|text| !text.is_empty()).collect();
    if details.is_empty() { name.to_owned() } else { format!("{name} ({})", details.join(", ")) }
}

/// `ChoiceState` is the `Choice` state.
fn context_name(context: &str) -> &str {
    context.strip_suffix("State").filter(|name| !name.is_empty()).unwrap_or(context)
}

fn count(number: i32, singular: &str, plural: &str) -> String {
    if number == 1 { format!("1 {singular}") } else { format!("{number} {plural}") }
}

#[cfg(test)]
#[path = "narrate_tests.rs"]
mod tests;
