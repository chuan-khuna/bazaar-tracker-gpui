//! The rows of the Events list: when, where in the day, what kind and a short description.

use bazaar_tracker::run::{Event, EventKind, Outcome};

use super::names::Names;
use super::when;

/// How a row is coloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Win,
    Loss,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRow {
    pub id: String,
    /// `1:05`, how far into the Run.
    pub time: String,
    /// `D2 H4` when the day changes, `H5` when only the hour does, otherwise empty.
    pub hour: String,
    pub kind: &'static str,
    pub description: String,
    /// A choice or a sale: drawn indented under the Event before it.
    pub sub: bool,
    pub tone: Tone,
    pub selected: bool,
}

/// Which Events the list shows: `None` is every day or every hour. An Event with no day or hour of
/// its own is counted in the last one before it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub day: Option<i32>,
    pub hour: Option<i32>,
}

/// The hours of a day that always have a button. An hour 0 gets one too when the Run has Events in it.
pub const HOURS: std::ops::RangeInclusive<i32> = 1..=6;

/// The days to offer as filters: 1 to the latest day of the Run.
pub fn days(events: &[Event]) -> Vec<i32> {
    let last = events.iter().filter_map(|event| event.day).max().unwrap_or(0);
    (1..=last).collect()
}

/// The hours to offer as filters: [`HOURS`], with 0 first when an Event happened in it.
pub fn hours(events: &[Event]) -> Vec<i32> {
    let zero = events.iter().any(|event| event.hour == Some(0));
    zero.then_some(0).into_iter().chain(HOURS).collect()
}

/// One row for each Event that `filter` lets through, in order. `selected` is the id of the chosen
/// Event, if any. The times are from the start of the Run, and the day and hour column marks a change
/// from the row above it.
pub fn rows(events: &[Event], selected: Option<&str>, filter: Filter, names: &Names) -> Vec<EventRow> {
    let started = events.first().map_or(0, |event| event.timestamp_ms);
    // The day and hour each Event is in, carried over Events that have none of their own.
    let (mut in_day, mut in_hour) = (None, None);
    // The day and hour of the row above, for the column.
    let (mut day, mut hour) = (None, None);
    events
        .iter()
        .filter(|event| {
            in_day = event.day.or(in_day);
            in_hour = event.hour.or(in_hour);
            filter.day.is_none_or(|wanted| in_day == Some(wanted)) && filter.hour.is_none_or(|wanted| in_hour == Some(wanted))
        })
        .map(|event| {
            let label = match (event.day, event.hour) {
                (Some(new_day), Some(new_hour)) if day != Some(new_day) => format!("D{new_day} H{new_hour}"),
                (_, Some(new_hour)) if hour != Some(new_hour) => format!("H{new_hour}"),
                _ => String::new(),
            };
            day = event.day.or(day);
            hour = event.hour.or(hour);
            let (description, tone) = describe(&event.kind, names);
            EventRow {
                id: event.id.clone(),
                time: when::into_run(event.timestamp_ms, started),
                hour: label,
                kind: kind_label(&event.kind),
                description,
                sub: matches!(event.kind, EventKind::Selected { .. } | EventKind::Sold { .. }),
                tone,
                selected: selected == Some(event.id.as_str()),
            }
        })
        .collect()
}

pub fn kind_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::RunStarted { .. } => "Run Started",
        EventKind::OptionsPresented { .. } => "Options",
        EventKind::Selected { .. } => "Selected",
        EventKind::Sold { .. } => "Sold",
        EventKind::Rerolled => "Rerolled",
        EventKind::CombatStart { .. } => "Combat Start",
        EventKind::CombatEnd { .. } => "Combat End",
        EventKind::RunEnded { .. } => "Run Ended",
    }
}

/// `Victory`, `Defeat` or `Ended`.
pub fn outcome_word(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Win => "Victory",
        Outcome::Loss => "Defeat",
        Outcome::Unknown => "Ended",
    }
}

fn tone_of(outcome: Outcome) -> Tone {
    match outcome {
        Outcome::Win => Tone::Win,
        Outcome::Loss => Tone::Loss,
        Outcome::Unknown => Tone::Plain,
    }
}

/// `ChoiceState` is the `Choice` state.
fn context_name(context: &str) -> &str {
    context.strip_suffix("State").filter(|name| !name.is_empty()).unwrap_or(context)
}

fn with_tier(text: String, tier: Option<&str>) -> String {
    match tier {
        Some(tier) => format!("{text} ({tier})"),
        None => text,
    }
}

fn describe(kind: &EventKind, names: &Names) -> (String, Tone) {
    let text = match kind {
        EventKind::RunStarted { hero, play_mode, .. } => [hero.as_deref(), play_mode.as_deref()].into_iter().flatten().collect::<Vec<_>>().join(" · "),
        EventKind::OptionsPresented { context, options, .. } => {
            let count = if options.len() == 1 { "1 option".to_owned() } else { format!("{} options", options.len()) };
            format!("{count} · {}", context_name(context))
        }
        EventKind::Selected { card_kind, instance_id, template_id, tier, .. } => format!("{}: {card_kind}", with_tier(names.card(template_id.as_deref(), instance_id), tier.as_deref())),
        EventKind::Sold { instance_id, template_id, tier, .. } => with_tier(names.card(template_id.as_deref(), instance_id), tier.as_deref()),
        EventKind::Rerolled => String::new(),
        EventKind::CombatStart { is_pvp, opponent_name, opponent_hero, .. } => {
            if !*is_pvp {
                "PvE".to_owned()
            } else {
                match (opponent_name, opponent_hero) {
                    (Some(name), Some(hero)) => format!("PvP vs {name} ({hero})"),
                    (Some(name), None) => format!("PvP vs {name}"),
                    (None, Some(hero)) => format!("PvP vs {hero}"),
                    (None, None) => "PvP".to_owned(),
                }
            }
        }
        EventKind::CombatEnd { outcome, .. } => return (outcome_word(*outcome).to_owned(), tone_of(*outcome)),
        EventKind::RunEnded { outcome, victories, losses, .. } => {
            let word = outcome_word(*outcome);
            let text = match (victories, losses) {
                (Some(victories), Some(losses)) => format!("{word} · {victories}W–{losses}L"),
                _ => word.to_owned(),
            };
            return (text, tone_of(*outcome));
        }
    };
    (text, Tone::Plain)
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
