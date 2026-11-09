//! An Event attached to the next message: picked with the chat icon on a row of the Run tab's Event
//! table, shown as a removable chip above the text in the Advisor's box. The message it is sent
//! with carries the situation at that Event (`situation::for_event`) instead of the situation now;
//! the messages after it go back to the situation now.

use bazaar_tracker::run::Event;

use super::conversation::time_label;
use crate::tracking::names::Names;
use crate::tracking::run::events::{describe, kind_label, placed};

/// The attached Event, of the Advisor's Run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub run_id: String,
    pub event_id: String,
    /// The chip's text: `Event #7 · D5 H2 · Options: 3 options · Choice`.
    pub label: String,
}

impl Attachment {
    /// The Event with id `event_id` among `run_id`'s `events` (in order), or `None` when it isn't
    /// among them.
    pub fn new(run_id: &str, events: &[Event], event_id: &str, names: &Names) -> Option<Self> {
        Some(Self { run_id: run_id.to_owned(), event_id: event_id.to_owned(), label: chip_text(events, event_id, names)? })
    }
}

/// `Event #7 · D5 H2 · Options: 3 options · Choice`: the Event's number in the Run (as the Run
/// tab's detail numbers it), its Day and Hour (as the table places it), its kind and the table's
/// detail, each part when there is one.
pub fn chip_text(events: &[Event], event_id: &str, names: &Names) -> Option<String> {
    let (index, (event, day, hour)) = placed(events).enumerate().find(|(_, (event, _, _))| event.id == event_id)?;
    let mut parts = vec![format!("Event #{}", index + 1)];
    parts.extend(time_label(day, hour));
    let (detail, _) = describe(&event.kind, names);
    let kind = kind_label(&event.kind);
    parts.push(if detail.is_empty() { kind.to_owned() } else { format!("{kind}: {detail}") });
    Some(parts.join(" · "))
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
