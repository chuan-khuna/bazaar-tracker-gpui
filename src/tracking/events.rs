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
    /// `D2` and `H4`: the day and hour the Event is in (empty before the Run has one).
    pub day: String,
    pub hour: String,
    /// Whether the day or hour is not the row above's: the table marks where a new hour begins.
    pub moved: bool,
    pub kind: &'static str,
    /// A tag after the kind: `PvP` or `PvE` for a combat start.
    pub badge: Option<&'static str>,
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
/// Event, if any. The times are from the start of the Run; every row has its day and hour, and says
/// whether they changed from the row above.
pub fn rows(events: &[Event], selected: Option<&str>, filter: Filter, names: &Names) -> Vec<EventRow> {
    let started = events.first().map_or(0, |event| event.timestamp_ms);
    // The day and hour each Event is in, carried over Events that have none of their own.
    let (mut day, mut hour) = (None, None);
    let placed: Vec<(&Event, Option<i32>, Option<i32>)> = events
        .iter()
        .map(|event| {
            day = event.day.or(day);
            hour = event.hour.or(hour);
            (event, day, hour)
        })
        .filter(|(_, day, hour)| filter.day.is_none_or(|wanted| *day == Some(wanted)) && filter.hour.is_none_or(|wanted| *hour == Some(wanted)))
        .collect();
    let mut above = None;
    placed
        .into_iter()
        .map(|(event, day, hour)| {
            let moved = above != Some((day, hour));
            above = Some((day, hour));
            let (description, tone) = describe(&event.kind, names);
            EventRow {
                id: event.id.clone(),
                time: when::into_run(event.timestamp_ms, started),
                day: day.map(|day| format!("D{day}")).unwrap_or_default(),
                hour: hour.map(|hour| format!("H{hour}")).unwrap_or_default(),
                moved,
                kind: kind_label(&event.kind),
                badge: badge(&event.kind),
                description,
                sub: matches!(event.kind, EventKind::Selected { .. } | EventKind::Sold { .. }),
                tone,
                selected: selected == Some(event.id.as_str()),
            }
        })
        .collect()
}

/// The tag shown after an Event's kind, if it has one.
fn badge(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::CombatStart { is_pvp: true, .. } => Some("PvP"),
        EventKind::CombatStart { is_pvp: false, .. } => Some("PvE"),
        _ => None,
    }
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
        // PvP or PvE is the row's badge; the description names the opponent.
        EventKind::CombatStart { is_pvp, opponent_name, opponent_hero, .. } => match (is_pvp, opponent_name, opponent_hero) {
            (false, _, _) | (true, None, None) => String::new(),
            (true, Some(name), Some(hero)) => format!("vs {name} ({hero})"),
            (true, Some(name), None) => format!("vs {name}"),
            (true, None, Some(hero)) => format!("vs {hero}"),
        },
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
