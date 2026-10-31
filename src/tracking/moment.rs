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

#[cfg(test)]
#[path = "moment_tests.rs"]
mod tests;
