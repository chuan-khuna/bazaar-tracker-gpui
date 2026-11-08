//! The items the player pins for a hero (schema migration 7): tracked in each of that hero's Runs,
//! on top of the items tracked in the Run itself. They belong to the hero, not a Run, so deleting a
//! Run leaves them.

use rusqlite::params;

use super::{History, Result};

impl History {
    /// The game's card ids of the items pinned for `hero`, lower-cased, in the order they were pinned.
    pub fn pinned_items(&self, hero: &str) -> Result<Vec<String>> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare("SELECT template_id FROM pinned_items WHERE hero = ?1 ORDER BY pinned_ms, template_id")?;
            let ids = statement.query_map([hero], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
            Ok(ids)
        })
    }

    /// Pin `template_id` for `hero`, pinned at `now_ms`. Pinning it again changes nothing.
    pub fn pin_item(&self, hero: &str, template_id: &str, now_ms: i64) -> Result<()> {
        self.with_writer(|connection| {
            connection.execute("INSERT OR IGNORE INTO pinned_items (hero, template_id, pinned_ms) VALUES (?1, ?2, ?3)", params![hero, template_id.to_lowercase(), now_ms])?;
            Ok(())
        })
    }

    /// Unpin `template_id` for `hero`.
    pub fn unpin_item(&self, hero: &str, template_id: &str) -> Result<()> {
        self.with_writer(|connection| {
            connection.execute("DELETE FROM pinned_items WHERE hero = ?1 AND template_id = ?2", params![hero, template_id.to_lowercase()])?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "pinned_items_tests.rs"]
mod tests;
