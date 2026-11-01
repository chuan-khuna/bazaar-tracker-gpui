//! The shop odds the app shows: the next few days of a Run beside its Economy, and every day in the
//! Shop odds widget. The odds come from the history (Bazaar DB's, or the built-in copy).

use bazaar_tracker::history::SavedShopOdds;
use bazaardb::{DayOdds, ShopOdds};

/// How many days are shown beside the Economy: the Run's day and the two after it.
pub const DAYS_AHEAD: u32 = 3;

/// One day's row.
#[derive(Clone, Debug, PartialEq)]
pub struct OddsRow {
    /// The day the row is for.
    pub day: u32,
    /// Percent, Bronze, Silver, Gold, Diamond. `None` when Bazaar DB has too little data for it.
    pub tiers: Option<[f32; 4]>,
    /// The Run's own day.
    pub today: bool,
    /// The day whose odds are shown, when it is not `day`: days past the last one with data show
    /// the last one's.
    pub borrowed_from: Option<u32>,
}

/// What is shown beside the Economy.
#[derive(Clone, Debug, PartialEq)]
pub struct Upcoming {
    pub rows: Vec<OddsRow>,
    /// Said under the rows when some days are past the last one with data.
    pub note: Option<String>,
}

/// The odds for `today` and the days after it, [`DAYS_AHEAD`] in all.
pub fn upcoming(odds: &ShopOdds, today: u32) -> Upcoming {
    let last = odds.days.last();
    let today = today.max(1);
    let rows: Vec<OddsRow> = (today..today + DAYS_AHEAD)
        .map(|day| match (odds.for_day(day), last) {
            (Some(found), _) => OddsRow { day, tiers: Some(found.tiers), today: day == today, borrowed_from: None },
            (None, Some(last)) if day > last.day => OddsRow { day, tiers: Some(last.tiers), today: day == today, borrowed_from: Some(last.day) },
            (None, _) => OddsRow { day, tiers: None, today: day == today, borrowed_from: None },
        })
        .collect();
    let note = rows
        .iter()
        .find_map(|row| row.borrowed_from)
        .map(|last| format!("Shop odds only have {last} days; later days show day {last}."));
    Upcoming { rows, note }
}

/// Every day, for the widget.
pub fn all_days(odds: &ShopOdds) -> Vec<OddsRow> {
    odds.days.iter().map(|DayOdds { day, tiers, .. }| OddsRow { day: *day, tiers: Some(*tiers), today: false, borrowed_from: None }).collect()
}

/// A tier's share as the widget writes it: whole percent, or "<1" for a sliver.
pub fn percent(value: f32) -> String {
    if value > 0.0 && value < 0.5 { "<1%".to_owned() } else { format!("{}%", value.round() as i32) }
}

/// Where the odds came from, for the widget: the built-in copy, or when a newer one was saved.
pub fn source(saved: &SavedShopOdds) -> String {
    match saved.saved_ms {
        None => "No odds saved".to_owned(),
        Some(_) if saved.odds == bazaardb::shop_odds::defaults() => "Built-in copy (11/1/2026)".to_owned(),
        Some(ms) => format!("Saved {}", super::when::long(ms)),
    }
}

#[cfg(test)]
#[path = "shop_odds_tests.rs"]
mod tests;
