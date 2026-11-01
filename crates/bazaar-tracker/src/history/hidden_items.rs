//! The items the player hid from the odds beside each merchant in a Run (schema migration 6). Like
//! the tracked items, they belong to the Run: they follow it when its id is rebound and go when it
//! is deleted.

use rusqlite::params;

use super::{History, Result};

impl History {
    /// The game's card ids of the items hidden in `run_id`, lower-cased, in the order they were hidden.
    pub fn hidden_items(&self, run_id: &str) -> Result<Vec<String>> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare("SELECT template_id FROM hidden_items WHERE run_id = ?1 ORDER BY hidden_ms, template_id")?;
            let ids = statement.query_map([run_id], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
            Ok(ids)
        })
    }

    /// Hide `template_id` in `run_id`, at `now_ms`. Hiding it again changes nothing.
    pub fn hide_item(&self, run_id: &str, template_id: &str, now_ms: i64) -> Result<()> {
        self.with_writer(|connection| {
            connection.execute("INSERT OR IGNORE INTO hidden_items (run_id, template_id, hidden_ms) VALUES (?1, ?2, ?3)", params![run_id, template_id.to_lowercase(), now_ms])?;
            Ok(())
        })
    }

    /// Show `template_id` in `run_id` again.
    pub fn unhide_item(&self, run_id: &str, template_id: &str) -> Result<()> {
        self.with_writer(|connection| {
            connection.execute("DELETE FROM hidden_items WHERE run_id = ?1 AND template_id = ?2", params![run_id, template_id.to_lowercase()])?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "hidden_items_tests.rs"]
mod tests;
