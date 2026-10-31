//! What the detail island shows for the selected Event: what is specific to it, such as the options
//! that were offered or the opponent's board.

use bazaar_tracker::memory::Snapshot;
use bazaar_tracker::run::{Event, EventKind, Outcome, PresentedOption};

use super::board::{Grid, SkillChip, Tier, board_grid, skill_chips};
use super::events::{kind_label, outcome_word};
use crate::tracking::names::Names;
use crate::tracking::when;

/// One card in a section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardView {
    pub name: String,
    /// `Item · Bronze`.
    pub meta: String,
    pub tier: Option<Tier>,
    pub picked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardSection {
    pub title: &'static str,
    pub cards: Vec<CardView>,
}

/// A board in a combat, drawn as the snapshot draws one: its skills over its items on the ten slots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardBox {
    pub title: &'static str,
    pub board: Grid,
    pub skills: Vec<SkillChip>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetailView {
    /// `Options · #12 · 9:20:11 PM`.
    pub title: String,
    /// `Picked 1 of 4`.
    pub banner: Option<String>,
    /// Label and value pairs.
    pub facts: Vec<(&'static str, String)>,
    pub sections: Vec<CardSection>,
    pub boards: Vec<BoardBox>,
}

/// The detail of the Event with id `selected` among `events`.
pub fn detail(events: &[Event], selected: &str, names: &Names) -> DetailView {
    let Some(index) = events.iter().position(|event| event.id == selected) else {
        return DetailView { title: "Event".into(), banner: Some("This event is no longer in the run.".into()), ..DetailView::default() };
    };
    let event = &events[index];
    let mut view = DetailView { title: format!("{} · #{} · {}", kind_label(&event.kind), index + 1, when::clock(event.timestamp_ms)), ..DetailView::default() };
    match &event.kind {
        EventKind::RunStarted { play_mode, build_id, environment, hero, rating, rank, rank_division } => {
            fact(&mut view, "Hero", hero.clone());
            fact(&mut view, "Mode", play_mode.clone());
            fact(&mut view, "Build", build_id.clone());
            fact(&mut view, "Environment", environment.clone());
            fact(&mut view, "Rank", rank.as_ref().map(|rank| rank_division.map_or_else(|| rank.clone(), |division| format!("{rank} {division}"))));
            fact(&mut view, "Rating", rating.map(|rating| rating.to_string()));
        }
        EventKind::OptionsPresented { options, .. } => {
            let picked = events[index + 1..].iter().find_map(|later| match &later.kind {
                EventKind::Selected { options_event_id: Some(from), instance_id, .. } if *from == event.id => Some(instance_id.as_str()),
                _ => None,
            });
            options_view(&mut view, options, picked, names);
        }
        EventKind::Selected { card_kind, instance_id, template_id, tier, enchantment, section, target, merged_into, options_event_id, .. } => {
            let from = options_event_id.as_deref().and_then(|id| events.iter().find(|candidate| candidate.id == id));
            match from.map(|from| &from.kind) {
                Some(EventKind::OptionsPresented { options, .. }) => options_view(&mut view, options, Some(instance_id), names),
                _ => {
                    let picked = PresentedOption { instance_id: instance_id.clone(), kind: card_kind.clone(), template_id: template_id.clone(), tier: tier.clone(), enchantment: enchantment.clone(), ..PresentedOption::default() };
                    view.sections.push(CardSection { title: "SELECTED", cards: vec![option_card(&picked, true, names)] });
                }
            }
            fact(&mut view, "Section", section.clone());
            fact(&mut view, "Target", target.clone());
            fact(&mut view, "Merged into", merged_into.clone());
        }
        EventKind::Sold { instance_id, template_id, tier, enchantment, .. } => {
            let sold = PresentedOption { instance_id: instance_id.clone(), kind: "Item".into(), template_id: template_id.clone(), tier: tier.clone(), enchantment: enchantment.clone(), ..PresentedOption::default() };
            view.sections.push(CardSection { title: "SOLD", cards: vec![option_card(&sold, false, names)] });
        }
        EventKind::Rerolled => view.banner = Some("The offers were refreshed.".into()),
        EventKind::CombatStart { player, opponent, is_pvp, opponent_name, opponent_hero, opponent_rating, opponent_level } => {
            fact(&mut view, "Mode", Some(if *is_pvp { "PvP" } else { "PvE" }.to_owned()));
            fact(&mut view, "Opponent", opponent_name.clone());
            fact(&mut view, "Hero", opponent_hero.clone());
            fact(&mut view, "Rating", opponent_rating.map(|rating| rating.to_string()));
            fact(&mut view, "Level", opponent_level.map(|level| level.to_string()));
            view.boards.push(board_box("OPPONENT", opponent, names));
            view.boards.push(board_box("PLAYER", player, names));
        }
        EventKind::CombatEnd { outcome, combat_start_event_id, player_hp_after, opponent_hp_after, victories_after, losses_after, sim_diag, .. } => {
            fact(&mut view, "Result", Some(outcome_word(*outcome).to_owned()));
            fact(&mut view, "Your health", player_hp_after.map(|hp| hp.to_string()));
            fact(&mut view, "Opponent health", opponent_hp_after.map(|hp| hp.to_string()));
            fact(&mut view, "Record", record(*victories_after, *losses_after));
            fact(&mut view, "Note", sim_diag.clone());
            let start = combat_start_event_id.as_deref().and_then(|id| events.iter().find(|candidate| candidate.id == id));
            if let Some(Event { kind: EventKind::CombatStart { opponent, .. }, .. }) = start {
                view.boards.push(board_box("OPPONENT", opponent, names));
            }
        }
        EventKind::RunEnded { outcome, victories, losses, gold, health_max, prestige, level, income, rating_after, rank_after, rank_division_after } => {
            fact(&mut view, "Result", Some(outcome_word(*outcome).to_owned()));
            fact(&mut view, "Record", record(*victories, *losses));
            fact(&mut view, "Gold", gold.map(|value| value.to_string()));
            fact(&mut view, "Max health", health_max.map(|value| value.to_string()));
            fact(&mut view, "Prestige", prestige.map(|value| value.to_string()));
            fact(&mut view, "Level", level.map(|value| value.to_string()));
            fact(&mut view, "Income", income.map(|value| value.to_string()));
            fact(&mut view, "Rank", rank_after.as_ref().map(|rank| rank_division_after.map_or_else(|| rank.clone(), |division| format!("{rank} {division}"))));
            fact(&mut view, "Rating", rating_after.map(|rating| rating.to_string()));
            if *outcome == Outcome::Unknown {
                view.banner = Some("The result of the run was not recorded.".into());
            }
        }
    }
    view
}

fn fact(view: &mut DetailView, label: &'static str, value: Option<String>) {
    if let Some(value) = value {
        view.facts.push((label, value));
    }
}

fn record(victories: Option<i32>, losses: Option<i32>) -> Option<String> {
    match (victories, losses) {
        (Some(victories), Some(losses)) => Some(format!("{victories}W–{losses}L")),
        _ => None,
    }
}

fn options_view(view: &mut DetailView, options: &[PresentedOption], picked: Option<&str>, names: &Names) {
    view.banner = Some(match picked {
        Some(_) => format!("Picked 1 of {}", options.len()),
        None => format!("{} offered, none picked", options.len()),
    });
    view.sections.push(CardSection { title: "OPTIONS", cards: options.iter().map(|option| option_card(option, picked == Some(option.instance_id.as_str()), names)).collect() });
}

fn option_card(option: &PresentedOption, picked: bool, names: &Names) -> CardView {
    let mut meta = vec![option.kind.clone()];
    meta.extend(option.tier.clone());
    meta.extend(option.enchantment.clone());
    CardView { name: names.card(option.template_id.as_deref(), &option.instance_id), meta: meta.join(" · "), tier: option.tier.as_deref().and_then(Tier::parse), picked }
}

fn board_box(title: &'static str, snapshot: &Snapshot, names: &Names) -> BoardBox {
    BoardBox { title, board: board_grid(&snapshot.board, names), skills: skill_chips(&snapshot.skills, names) }
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod tests;
