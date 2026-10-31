//! The Runs the sidebar lists, which one is live and which one is selected, kept up to date from the
//! tracker's notices.
//!
//! The list is read from `history.db` once. After that each saved Event re-reads its own Run's row
//! from the history (so the summary rules live only in the history store) and puts it in place.

use bazaar_tracker::history::{self, History};
use bazaar_tracker::memory::ConnectionChange;
use bazaar_tracker::run::{Event, EventKind, Outcome, Run};
use bazaar_tracker::tracker::Notice;

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

    /// The id of the Run being played now.
    #[cfg(test)]
    pub fn live(&self) -> Option<&str> {
        self.live.as_deref()
    }

    #[cfg(test)]
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Select the Run with this id, if there is one.
    pub fn select(&mut self, run_id: &str) {
        if self.runs.iter().any(|run| run.id == run_id) {
            self.selected = Some(run_id.to_owned());
        }
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
            Notice::Connection(ConnectionChange::Connected { .. }) | Notice::SaveFailed { .. } | Notice::LogError(_) => Ok(()),
        }
    }

    fn event_saved(&mut self, event: &Event, history: &History) -> history::Result<()> {
        if let Some(run) = history.get_run(&event.run_id)? {
            self.upsert(run);
        }
        if matches!(event.kind, EventKind::RunEnded { .. }) {
            if self.live.as_deref() == Some(event.run_id.as_str()) {
                self.live = None;
            }
        } else if self.live.as_deref() != Some(event.run_id.as_str()) {
            self.live = Some(event.run_id.clone());
            self.select(&event.run_id);
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
