//! The Run's economy: the gold the player had at the start of each hour, for the Events table and
//! the line chart over it.

use bazaar_tracker::run::Event;

use super::events::placed;
use super::moment::snapshot_of;

/// The gold at the start of one hour: the gold of the first Snapshot saved in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HourGold {
    pub day: i32,
    pub hour: i32,
    pub gold: i32,
    /// What the gold grows by each day, when it was read.
    pub income: Option<i32>,
}

/// One point for each hour of the Run in which a Snapshot with gold was saved, in order. An Event
/// with no day or hour of its own counts in the last one before it, as in the Events table.
pub fn by_hour(events: &[Event]) -> Vec<HourGold> {
    let mut points: Vec<HourGold> = Vec::new();
    for (event, day, hour) in placed(events) {
        let (Some(day), Some(hour)) = (day, hour) else { continue };
        if points.last().is_some_and(|last| (last.day, last.hour) == (day, hour)) {
            continue;
        }
        if let Some(stats) = snapshot_of(event).map(|snapshot| &snapshot.stats)
            && let Some(gold) = stats.gold
        {
            points.push(HourGold { day, hour, gold, income: stats.income });
        }
    }
    points
}

#[cfg(test)]
#[path = "economy_tests.rs"]
mod tests;
