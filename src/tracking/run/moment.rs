//! Which Snapshot the board, stash and skills show: the moment the player is looking at.
//!
//! Only some Events carry a Snapshot of their own: the options offered, a selection, a sale, and
//! the player's side of a combat's start. For any other Event the nearest one before it is shown, so
//! the board is never blank in the middle of a Run.

use bazaar_tracker::memory::Snapshot;
use bazaar_tracker::run::{Event, EventKind};

/// The Snapshot an Event was saved with, if it has one.
pub fn snapshot_of(event: &Event) -> Option<&Snapshot> {
    match &event.kind {
        EventKind::OptionsPresented { snapshot, .. } | EventKind::Selected { snapshot, .. } | EventKind::Sold { snapshot, .. } => Some(snapshot),
        EventKind::CombatStart { player, .. } => Some(player),
        EventKind::RunStarted { .. } | EventKind::Rerolled | EventKind::CombatEnd { .. } | EventKind::RunEnded { .. } => None,
    }
}

/// The Snapshot to show for `events` (in the order they happened). With no Event selected, that is
/// the latest one: the live board of a live Run, the last board of a finished one. With one
/// selected, it is the nearest at or before that Event. An id that is not among the `events` counts
/// as no selection.
pub fn at<'a>(events: &'a [Event], selected: Option<&str>) -> Option<&'a Snapshot> {
    let end = selected.and_then(|id| events.iter().position(|event| event.id == id)).map_or(events.len(), |index| index + 1);
    events[..end].iter().rev().find_map(snapshot_of)
}

/// The Snapshot of the last Event on or before `day` that has one: the end of that day, or the
/// nearest before it if nothing was saved with a Snapshot that day. Events with no day are passed over.
pub fn at_day(events: &[Event], day: i32) -> Option<&Snapshot> {
    events.iter().rev().filter(|event| event.day.is_some_and(|event_day| event_day <= day)).find_map(snapshot_of)
}

/// What the player chose to look at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice<'a> {
    /// The latest moment.
    Latest,
    /// The moment of the Event with this id.
    Event(&'a str),
    /// The end of this day.
    Day(i32),
}

/// The Snapshot for a choice.
pub fn choose<'a>(events: &'a [Event], choice: Choice<'_>) -> Option<&'a Snapshot> {
    match choice {
        Choice::Latest => at(events, None),
        Choice::Event(id) => at(events, Some(id)),
        Choice::Day(day) => at_day(events, day),
    }
}

#[cfg(test)]
#[path = "moment_tests.rs"]
mod tests;
