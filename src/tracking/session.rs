//! The Runs the sidebar lists, which one is live and which one is selected, kept up to date from the
//! tracker's notices.
//!
//! The list is read from `history.db` once. After that each saved Event re-reads its own Run's row
//! from the history (so the summary rules live only in the history store) and puts it in place.

use bazaar_tracker::history::{self, History};
use bazaar_tracker::memory::ConnectionChange;
use bazaar_tracker::run::{Event, EventKind, Outcome, Run};
use bazaar_tracker::tracker::Notice;

use super::events::Filter;
use super::moment::Choice;
use super::when;

/// One row of the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRow {
    pub id: String,
    /// `10/30 9:14 PM`, local time.
    pub started: String,
    pub hero: String,
    /// `7W–2L`.
    pub record: String,
    /// Colours the record: a Run with no outcome yet has none.
    pub outcome: Option<Outcome>,
    /// `Day 7`, once the Run has a day.
    pub day: Option<String>,
    pub live: bool,
    pub selected: bool,
}

/// The Runs, the live one and the selected one.
#[derive(Debug, Default)]
pub struct Session {
    /// Newest first.
    runs: Vec<Run>,
    live: Option<String>,
    selected: Option<String>,
    /// The selected Run's Events, in the order they happened.
    events: Vec<Event>,
    /// The Event chosen in the list, in the selected Run.
    selected_event: Option<String>,
    /// The day chosen with the day buttons, or the day of the chosen Event.
    selected_day: Option<i32>,
    /// Which of the selected Run's Events the list shows.
    filter: Filter,
}

impl Session {
    /// The Runs in `history`, newest first. Nothing is live or selected.
    pub fn load(history: &History) -> history::Result<Self> {
        Ok(Self { runs: history.list_runs()?, ..Self::default() })
    }

    #[cfg(test)]
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// Whether the Run with this id is the one being played now.
    pub fn is_live(&self, run_id: &str) -> bool {
        self.live.as_deref() == Some(run_id)
    }

    pub fn live(&self) -> Option<&str> {
        self.live.as_deref()
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn selected_run(&self) -> Option<&Run> {
        let id = self.selected.as_deref()?;
        self.runs.iter().find(|run| run.id == id)
    }

    /// The selected Run's Events, in the order they happened.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The id of the chosen Event, if one is chosen.
    pub fn selected_event(&self) -> Option<&str> {
        self.selected_event.as_deref()
    }

    /// The day being looked at: the chosen day, or the day of the chosen Event.
    pub fn selected_day(&self) -> Option<i32> {
        self.selected_day
    }

    /// Which of the selected Run's Events the list shows.
    pub fn filter(&self) -> Filter {
        self.filter
    }

    /// Show only the Events of this day, or of every day (`None`).
    pub fn filter_day(&mut self, day: Option<i32>) {
        self.filter.day = day;
    }

    /// Show only the Events of this hour, or of every hour (`None`).
    pub fn filter_hour(&mut self, hour: Option<i32>) {
        self.filter.hour = hour;
    }

    /// What the board section shows: the chosen Event's moment, the chosen day's, or the latest.
    pub fn choice(&self) -> Choice<'_> {
        match (&self.selected_event, self.selected_day) {
            (Some(id), _) => Choice::Event(id),
            (None, Some(day)) => Choice::Day(day),
            (None, None) => Choice::Latest,
        }
    }

    /// Choose the Event with this id of the selected Run, or, if it is already chosen, let go of
    /// it so that the latest moment shows again. The day moves to the Event's day (and is let go
    /// of with it).
    pub fn toggle_event(&mut self, event_id: &str) {
        if self.selected_event.as_deref() == Some(event_id) {
            self.selected_event = None;
            self.selected_day = None;
        } else if let Some(event) = self.events.iter().find(|event| event.id == event_id) {
            self.selected_day = event.day;
            self.selected_event = Some(event_id.to_owned());
        }
    }

    /// Choose this day of the selected Run, or, if it is already chosen, let go of it so that the
    /// latest moment shows again. Choosing a day lets go of the chosen Event.
    pub fn toggle_day(&mut self, day: i32) {
        self.selected_event = None;
        self.selected_day = if self.selected_day == Some(day) { None } else { Some(day) };
    }

    /// Select the Run with this id, if there is one, and load its Events.
    pub fn select(&mut self, run_id: &str, history: &History) -> history::Result<()> {
        if self.runs.iter().any(|run| run.id == run_id) {
            self.events = history.list_events(run_id)?;
            if self.selected.as_deref() != Some(run_id) {
                self.selected_event = None;
                self.selected_day = None;
                self.filter = Filter::default();
            }
            self.selected = Some(run_id.to_owned());
        }
        Ok(())
    }

    /// The sidebar's rows, in order.
    pub fn rows(&self) -> Vec<RunRow> {
        self.runs
            .iter()
            .map(|run| RunRow {
                id: run.id.clone(),
                started: when::short(run.started_ms),
                hero: run.hero.clone().unwrap_or_else(|| "Unknown hero".into()),
                record: format!("{}W–{}L", run.victories.unwrap_or(0), run.losses.unwrap_or(0)),
                outcome: run.outcome.filter(|outcome| *outcome != Outcome::Unknown),
                day: run.day.map(|day| format!("Day {day}")),
                live: self.live.as_deref() == Some(run.id.as_str()),
                selected: self.selected.as_deref() == Some(run.id.as_str()),
            })
            .collect()
    }

    /// Take a notice in. A Run that becomes live is selected.
    pub fn apply(&mut self, notice: &Notice, history: &History) -> history::Result<()> {
        match notice {
            Notice::EventSaved(event) => self.event_saved(event, history),
            Notice::RunRebound { old_run_id, new_run_id } => self.rebound(old_run_id, new_run_id, history),
            Notice::Connection(ConnectionChange::Disconnected { .. }) => {
                self.live = None;
                Ok(())
            }
            Notice::Connection(ConnectionChange::Connected { .. }) | Notice::SaveFailed { .. } | Notice::LogError(_) | Notice::LiveSnapshot(_) => Ok(()),
        }
    }

    fn event_saved(&mut self, event: &Event, history: &History) -> history::Result<()> {
        if let Some(run) = history.get_run(&event.run_id)? {
            self.upsert(run);
        }
        if matches!(event.kind, EventKind::RunEnded { .. }) {
            if self.is_live(&event.run_id) {
                self.live = None;
            }
        } else if !self.is_live(&event.run_id) {
            self.live = Some(event.run_id.clone());
            // The Events read from the history include this one.
            return self.select(&event.run_id, history);
        }
        if self.selected.as_deref() == Some(event.run_id.as_str()) && !self.events.iter().any(|known| known.id == event.id) {
            self.events.push(event.clone());
        }
        Ok(())
    }

    fn rebound(&mut self, old: &str, new: &str, history: &History) -> history::Result<()> {
        self.runs.retain(|run| run.id != old);
        if let Some(run) = history.get_run(new)? {
            self.upsert(run);
        }
        for slot in [&mut self.live, &mut self.selected] {
            if slot.as_deref() == Some(old) {
                *slot = Some(new.to_owned());
            }
        }
        if self.selected.as_deref() == Some(new) {
            self.events = history.list_events(new)?;
        }
        Ok(())
    }

    /// Replace the Run with this id, or add it, keeping the newest first.
    fn upsert(&mut self, run: Run) {
        match self.runs.iter().position(|known| known.id == run.id) {
            Some(index) => {
                self.runs[index] = run;
                self.runs.sort_by_key(|run| std::cmp::Reverse(run.started_ms));
            }
            None => {
                let index = self.runs.partition_point(|known| known.started_ms > run.started_ms);
                self.runs.insert(index, run);
            }
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
