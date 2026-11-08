//! Which Events have been uploaded, and which the server refused for good (schema migration 2).

use rusqlite::params;

use super::{History, Result, event_from_row, event_row};
use crate::run::Event;

/// Where an Event is in the upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadStatus {
    /// Not uploaded yet.
    Waiting,
    /// The server accepted it.
    Sent,
    /// The server refused it for good: it is set aside and never tried again.
    Rejected,
}

impl UploadStatus {
    fn code(self) -> i64 {
        match self {
            Self::Waiting => 0,
            Self::Sent => 1,
            Self::Rejected => 3,
        }
    }

    fn from_code(code: i64) -> Self {
        match code {
            1 => Self::Sent,
            3 => Self::Rejected,
            _ => Self::Waiting,
        }
    }
}

/// An Event that can be uploaded, with the size of its JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingEvent {
    pub event: Event,
    /// The Event serialised, as it will be sent.
    pub json: String,
}

impl History {
    /// The Events that can be uploaded now, oldest first, at most `limit`. An Event can be uploaded
    /// when it has not been, its Run has a real id (not `local-`), and its Run has ended or has had
    /// no new Event since `stale_before_ms`.
    pub fn waiting_events(&self, limit: usize, stale_before_ms: i64) -> Result<Vec<WaitingEvent>> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare(
                "SELECT e.id, e.run_id, e.timestamp_ms, e.day, e.hour, e.details
                 FROM run_tracker_events e JOIN run_tracker_runs r ON r.id = e.run_id
                 WHERE e.upload_status = 0
                   AND e.run_id NOT LIKE 'local-%'
                   AND (r.ended = 1 OR (SELECT MAX(e2.timestamp_ms) FROM run_tracker_events e2 WHERE e2.run_id = r.id) <= ?1)
                 ORDER BY e.seq
                 LIMIT ?2",
            )?;
            let rows = statement.query_map(params![stale_before_ms, limit as i64], event_row)?.collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter()
                .map(|row| {
                    let event = event_from_row(row)?;
                    let json = serde_json::to_string(&event)?;
                    Ok(WaitingEvent { event, json })
                })
                .collect()
        })
    }

    /// Record what became of these Events.
    pub fn set_upload_status(&self, event_ids: &[String], status: UploadStatus) -> Result<()> {
        self.with_writer(|connection| {
            let transaction = connection.transaction()?;
            {
                let mut statement = transaction.prepare("UPDATE run_tracker_events SET upload_status = ?2 WHERE id = ?1")?;
                for id in event_ids {
                    statement.execute(params![id, status.code()])?;
                }
            }
            transaction.commit()?;
            Ok(())
        })
    }

    /// Where an Event is in the upload, or `None` if there is no such Event.
    pub fn upload_status(&self, event_id: &str) -> Result<Option<UploadStatus>> {
        use rusqlite::OptionalExtension;
        self.with_reader(|connection| Ok(connection.query_row("SELECT upload_status FROM run_tracker_events WHERE id = ?1", [event_id], |row| row.get::<_, i64>(0)).optional()?.map(UploadStatus::from_code)))
    }
}
