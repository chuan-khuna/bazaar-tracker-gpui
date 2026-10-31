//! The line of facts above the board: when the Run started, who played it, how it stands and how
//! each day went.

use bazaar_tracker::run::{Event, EventKind, Outcome, Run};

use super::when;

/// How one day of the Run went, by its last combat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DayNode {
    pub day: i32,
    /// `None` for a day with no combat that has ended.
    pub result: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// `10/30/2026 9:14 PM`, local time.
    pub started: String,
    pub hero: Option<String>,
    /// `Gold 3 · 812`: the standing the Run began with.
    pub rank: Option<String>,
    /// `+24`: how the rating changed over the Run, once it ended.
    pub rating_change: Option<String>,
    pub live: bool,
    /// How the Run ended. `None` while it has not.
    pub outcome: Option<Outcome>,
    /// `Day 7`.
    pub day: Option<String>,
    /// `7W–2L`.
    pub record: String,
    pub timeline: Vec<DayNode>,
}

/// The header for `run`, from its `events` (in order). `live` says whether it is the live Run.
pub fn header(run: &Run, events: &[Event], live: bool) -> Header {
    let (rating, rank, division) = events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::RunStarted { rating, rank, rank_division, .. } => Some((*rating, rank.clone(), *rank_division)),
            _ => None,
        })
        .unwrap_or((None, None, None));
    let rating_after = events.iter().rev().find_map(|event| match &event.kind {
        EventKind::RunEnded { rating_after, .. } => *rating_after,
        _ => None,
    });
    Header {
        started: when::long(run.started_ms),
        hero: run.hero.clone(),
        rank: standing(rating, rank, division),
        rating_change: rating.zip(rating_after).map(|(before, after)| format!("{:+}", after - before)),
        live,
        outcome: run.outcome.filter(|outcome| *outcome != Outcome::Unknown),
        day: run.day.map(|day| format!("Day {day}")),
        record: format!("{}W–{}L", run.victories.unwrap_or(0), run.losses.unwrap_or(0)),
        timeline: timeline(run, events),
    }
}

fn standing(rating: Option<i32>, rank: Option<String>, division: Option<i32>) -> Option<String> {
    let rank = rank.map(|rank| match division {
        Some(division) => format!("{rank} {division}"),
        None => rank,
    });
    match (rank, rating) {
        (Some(rank), Some(rating)) => Some(format!("{rank} · {rating}")),
        (Some(rank), None) => Some(rank),
        (None, Some(rating)) => Some(rating.to_string()),
        (None, None) => None,
    }
}

/// A node for each day from 1 to the last the Run reached, marked by that day's last combat.
fn timeline(run: &Run, events: &[Event]) -> Vec<DayNode> {
    let last = events.iter().filter_map(|event| event.day).chain(run.day).max().unwrap_or(0);
    (1..=last)
        .map(|day| DayNode {
            day,
            result: events
                .iter()
                .rev()
                .filter(|event| event.day == Some(day))
                .find_map(|event| match &event.kind {
                    EventKind::CombatEnd { outcome, .. } if *outcome != Outcome::Unknown => Some(*outcome),
                    _ => None,
                }),
        })
        .collect()
}

#[cfg(test)]
#[path = "header_tests.rs"]
mod tests;
