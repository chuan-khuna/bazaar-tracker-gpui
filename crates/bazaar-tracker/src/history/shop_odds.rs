//! Bazaar DB's shop odds, kept in the history so they are there offline (schema migration 3).
//!
//! The table holds one copy: saving new odds replaces every day. A new file starts with
//! [`bazaardb::shop_odds::defaults`].

use bazaardb::{DayOdds, ShopOdds};
use rusqlite::params;

use super::{History, Result};

/// The shop odds in the history, and when they were saved.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedShopOdds {
    pub odds: ShopOdds,
    /// When they were saved, in milliseconds since the Unix epoch; `None` when there are none.
    pub saved_ms: Option<i64>,
}

impl History {
    /// The saved shop odds, in day order.
    pub fn shop_odds(&self) -> Result<SavedShopOdds> {
        self.with_reader(|connection| {
            let mut statement =
                connection.prepare("SELECT day, samples, bronze, silver, gold, diamond, saved_ms FROM shop_odds ORDER BY day")?;
            let mut saved_ms = None;
            let days = statement
                .query_map([], |row| {
                    Ok((
                        DayOdds {
                            day: row.get(0)?,
                            samples: row.get::<_, i64>(1)?.max(0) as u64,
                            tiers: [row.get::<_, f64>(2)? as f32, row.get::<_, f64>(3)? as f32, row.get::<_, f64>(4)? as f32, row.get::<_, f64>(5)? as f32],
                        },
                        row.get::<_, i64>(6)?,
                    ))
                })?
                .map(|row| {
                    row.map(|(day, at)| {
                        saved_ms = Some(saved_ms.map_or(at, |seen: i64| seen.max(at)));
                        day
                    })
                })
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(SavedShopOdds { odds: ShopOdds { days }, saved_ms })
        })
    }

    /// Replace the saved shop odds with `odds`, saved at `now_ms`, in one transaction.
    pub fn save_shop_odds(&self, odds: &ShopOdds, now_ms: i64) -> Result<()> {
        self.with_writer(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute("DELETE FROM shop_odds", [])?;
            {
                let mut insert = transaction.prepare(
                    "INSERT OR REPLACE INTO shop_odds (day, samples, bronze, silver, gold, diamond, saved_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )?;
                for day in &odds.days {
                    let [bronze, silver, gold, diamond] = day.tiers.map(f64::from);
                    insert.execute(params![day.day, day.samples as i64, bronze, silver, gold, diamond, now_ms])?;
                }
            }
            transaction.commit()?;
            Ok(())
        })
    }

    /// Save `odds` at `now_ms` unless some are saved already. Returns whether they were saved.
    pub fn seed_shop_odds(&self, odds: &ShopOdds, now_ms: i64) -> Result<bool> {
        let empty = self.with_reader(|connection| {
            Ok(connection.query_row("SELECT COUNT(*) FROM shop_odds", [], |row| row.get::<_, i64>(0))? == 0)
        })?;
        if empty {
            self.save_shop_odds(odds, now_ms)?;
        }
        Ok(empty)
    }
}

#[cfg(test)]
#[path = "shop_odds_tests.rs"]
mod tests;
