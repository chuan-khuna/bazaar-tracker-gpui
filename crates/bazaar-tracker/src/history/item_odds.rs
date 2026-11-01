//! Bazaar DB's odds for single items, kept in the history so they are there offline (schema
//! migration 4).
//!
//! Each row is one item, for one hero and the tier the player owns it at (`None` when not owned),
//! with Bazaar DB's answer kept as it came: it is read with [`bazaardb::item_odds::parse_json`]. Only
//! odds the player asked for are saved; saving the same item, hero and tier again replaces them.

use rusqlite::params;

use super::{History, Result};

/// One item's saved odds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedItemOdds {
    /// The game's id for the card (its template id), lower-cased.
    pub template_id: String,
    pub hero: String,
    /// The tier the player owns the item at, or `None`.
    pub tier: String,
    /// The item's Bazaar DB id, so a refresh can name it to the shop odds page.
    pub url_id: String,
    pub item_name: String,
    /// Bazaar DB's answer.
    pub body: String,
    /// When they were saved, in milliseconds since the Unix epoch.
    pub saved_ms: i64,
}

impl History {
    /// Every item's saved odds.
    pub fn item_odds(&self) -> Result<Vec<SavedItemOdds>> {
        self.with_reader(|connection| {
            let mut statement = connection.prepare("SELECT template_id, hero, tier, url_id, item_name, body, saved_ms FROM item_odds ORDER BY template_id, hero, tier")?;
            let rows = statement
                .query_map([], |row| {
                    Ok(SavedItemOdds { template_id: row.get(0)?, hero: row.get(1)?, tier: row.get(2)?, url_id: row.get(3)?, item_name: row.get(4)?, body: row.get(5)?, saved_ms: row.get(6)? })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    /// Save `odds`, replacing any saved for the same item, hero and tier.
    pub fn save_item_odds(&self, odds: &SavedItemOdds) -> Result<()> {
        self.with_writer(|connection| {
            connection.execute(
                "INSERT OR REPLACE INTO item_odds (template_id, hero, tier, url_id, item_name, body, saved_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![odds.template_id.to_lowercase(), odds.hero, odds.tier, odds.url_id, odds.item_name, odds.body, odds.saved_ms],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "item_odds_tests.rs"]
mod tests;
