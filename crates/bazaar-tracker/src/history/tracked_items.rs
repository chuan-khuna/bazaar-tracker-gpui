//! The items the player tracks in a Run (schema migration 5): items they want to see the odds of
//! beside each merchant, owned or not. They belong to the Run: they follow it when its id is
//! rebound and go when it is deleted.

use rusqlite::params;

use super::{History, Result};

impl History {
    /// The game's card ids of the items tracked in `run_id`, lower-cased, in the order they were added.
    pub fn tracked_items(&self, run_id: &str) -> Result<Vec<String>> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare("SELECT template_id FROM tracked_items WHERE run_id = ?1 ORDER BY added_ms, template_id")?;
            let ids = statement.query_map([run_id], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
            Ok(ids)
        })
    }

    /// Track `template_id` in `run_id`, added at `now_ms`. Tracking it again changes nothing.
    pub fn track_item(&self, run_id: &str, template_id: &str, now_ms: i64) -> Result<()> {
        self.with_writer(|connection| {
            connection.execute("INSERT OR IGNORE INTO tracked_items (run_id, template_id, added_ms) VALUES (?1, ?2, ?3)", params![run_id, template_id.to_lowercase(), now_ms])?;
            Ok(())
        })
    }

    /// Stop tracking `template_id` in `run_id`.
    pub fn untrack_item(&self, run_id: &str, template_id: &str) -> Result<()> {
        self.with_writer(|connection| {
            connection.execute("DELETE FROM tracked_items WHERE run_id = ?1 AND template_id = ?2", params![run_id, template_id.to_lowercase()])?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "tracked_items_tests.rs"]
mod tests;
