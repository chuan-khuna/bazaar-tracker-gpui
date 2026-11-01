//! The shop odds: for each day, the share in percent of items offered at merchants that are of
//! each tier, from Bazaar DB's community-tracked runs.
//!
//! The page (https://bazaardb.gg/shop-odds) has no data in its HTML; it loads it in the browser
//! from `/api/shop-odds`, whose answer looks like:
//!
//! ```json
//! { "days": [ { "day": 1, "samples": 51234, "tiers": { "Bronze": 88.1, "Silver": 11.9 } } ] }
//! ```
//!
//! A tier missing from `tiers` is 0. A day without enough data is missing from `days`.

use serde::{Deserialize, Serialize};

/// The tiers a shop can offer, in the order [`DayOdds::tiers`] holds them (shops never offer
/// Legendary, and the page leaves it out).
pub const TIERS: [&str; 4] = ["Bronze", "Silver", "Gold", "Diamond"];

/// One day's odds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DayOdds {
    pub day: u32,
    /// How many offered items the odds are counted from.
    pub samples: u64,
    /// Percent, in [`TIERS`] order.
    pub tiers: [f32; 4],
}

/// Every day Bazaar DB has data for, in day order.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ShopOdds {
    pub days: Vec<DayOdds>,
}

impl ShopOdds {
    /// The odds for `day`, if Bazaar DB has data for it.
    pub fn for_day(&self, day: u32) -> Option<&DayOdds> {
        self.days.iter().find(|d| d.day == day)
    }

    /// The last day with data.
    pub fn last_day(&self) -> Option<u32> {
        self.days.last().map(|d| d.day)
    }
}

/// The odds as bazaardb.gg/shop-odds showed them on 2026-11-01 (patch 19.4), for use until a
/// newer copy is saved. Day 6's 1% Diamond is moved to Gold: shops do not offer Diamond yet on day
/// 6, so it is taken for noise in the tracked data.
pub fn defaults() -> ShopOdds {
    // (day, samples, [Bronze, Silver, Gold, Diamond])
    const DAYS: [(u32, u64, [f32; 4]); 16] = [
        (1, 120_884, [97.0, 3.0, 0.0, 0.0]),
        (2, 116_626, [88.0, 12.0, 0.0, 0.0]),
        (3, 115_287, [69.0, 31.0, 0.0, 0.0]),
        (4, 115_503, [49.0, 51.0, 0.0, 0.0]),
        (5, 128_989, [25.0, 75.0, 0.0, 0.0]),
        (6, 117_783, [1.0, 91.0, 8.0, 0.0]),
        (7, 127_345, [1.0, 79.0, 20.0, 0.0]),
        (8, 131_978, [0.0, 59.0, 40.0, 0.0]),
        (9, 125_514, [0.0, 35.0, 60.0, 5.0]),
        (10, 116_753, [0.0, 21.0, 69.0, 10.0]),
        (11, 95_070, [0.0, 21.0, 69.0, 10.0]),
        (12, 66_526, [0.0, 21.0, 69.0, 10.0]),
        (13, 40_449, [0.0, 21.0, 69.0, 10.0]),
        (14, 18_214, [0.0, 21.0, 69.0, 10.0]),
        (15, 5_788, [0.0, 21.0, 69.0, 10.0]),
        (16, 1_075, [0.0, 19.0, 71.0, 10.0]),
    ];
    ShopOdds { days: DAYS.iter().map(|&(day, samples, tiers)| DayOdds { day, samples, tiers }).collect() }
}

/// Why the text held no shop odds.
#[derive(Debug)]
pub enum ParseError {
    /// The text is Cloudflare's challenge or the API's refusal, not the odds.
    Blocked,
    /// It is not the JSON `/api/shop-odds` answers with.
    Json(serde_json::Error),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Blocked => write!(f, "this is Bazaar DB's bot check or a refusal, not the shop odds"),
            ParseError::Json(error) => write!(f, "not the shop odds JSON: {error}"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    days: Vec<RawDay>,
}

#[derive(Deserialize)]
struct RawDay {
    day: u32,
    #[serde(default)]
    samples: u64,
    #[serde(default)]
    tiers: std::collections::HashMap<String, f32>,
}

/// Reads the answer of `/api/shop-odds`.
pub fn parse_json(text: &str) -> Result<ShopOdds, ParseError> {
    let trimmed = text.trim();
    if crate::fetch::is_challenge(trimmed) || trimmed == "Unauthorized" {
        return Err(ParseError::Blocked);
    }
    let answer: Answer = serde_json::from_str(trimmed).map_err(ParseError::Json)?;
    let mut days: Vec<DayOdds> = answer
        .days
        .into_iter()
        .map(|raw| DayOdds {
            day: raw.day,
            samples: raw.samples,
            tiers: TIERS.map(|tier| {
                raw.tiers.iter().find(|(k, _)| k.eq_ignore_ascii_case(tier)).map_or(0.0, |(_, v)| *v)
            }),
        })
        .collect();
    days.sort_by_key(|d| d.day);
    days.dedup_by_key(|d| d.day);
    Ok(ShopOdds { days })
}

#[cfg(test)]
#[path = "shop_odds_tests.rs"]
mod tests;
