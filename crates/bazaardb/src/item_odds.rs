//! One item's odds, from `/api/item-odds`: for each merchant and each day, the chance the item is
//! in stock when the player visits that merchant and uses its one reroll.
//!
//! The shop odds page asks for them when an item is picked, with the item's Bazaar DB id, the hero
//! and the tier the player owns it at (`None` when not owned). The answer's shape is read from the
//! page's own script (it was not documented); every field is optional here, so a field the page
//! adds or drops later does not stop the rest being read:
//!
//! ```json
//! { "itemName": "Pasta Maker", "itemUrlId": "qnwx…", "hero": "Jules", "tierOffered": "Bronze",
//!   "itemTiers": ["Bronze", "Silver", "Gold"], "heroOptions": ["Jules", "Common"], "itemHeroes": ["Jules"],
//!   "days": ["1", "2", "10+"], "ownedDataByDay": [true, true, false],
//!   "pAnyByDay": [0.1, 0.2, 0.0], "pAnyRerollByDay": [0.15, 0.3, 0.0],
//!   "merchants": [ { "name": "Chris", "uri": "/card/…", "alwaysTier": null,
//!                    "days": [ { "pOfferReroll": 0.05, "pSeen": 0.4 } ] } ] }
//! ```

use serde::Deserialize;
use serde_json::Value;

use crate::query;

/// Where the shop odds page gets one item's odds from.
pub const API_PATH: &str = "/api/item-odds";

/// What `/api/item-odds` answered for one item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemOdds {
    /// The item's Bazaar DB id, the hero and the tier asked about, from the request.
    pub item: String,
    pub hero: String,
    pub tier: String,
    /// The answer, unparsed.
    pub body: String,
}

impl ItemOdds {
    /// The answer `body` to a request for `url`.
    pub fn from_reply(url: &str, body: String) -> Self {
        Self { item: query::param(url, "item"), hero: query::param(url, "hero"), tier: query::param(url, "tier"), body }
    }

    /// The answer, read.
    pub fn table(&self) -> Result<ItemOddsTable, serde_json::Error> {
        parse_json(&self.body)
    }
}

/// One merchant's row: the chance the item is in stock there on each day.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MerchantOdds {
    pub name: String,
    /// The tier this merchant always sells at, if it has one.
    pub always_tier: Option<String>,
    /// In the order of [`ItemOddsTable::days`].
    pub days: Vec<MerchantDay>,
}

/// One merchant on one day. Chances are from 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MerchantDay {
    /// In stock after the merchant's one reroll.
    pub offer_reroll: Option<f64>,
    /// How often the merchant is presented as an option that day.
    pub seen: Option<f64>,
}

/// The answer of `/api/item-odds`, read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemOddsTable {
    pub item_name: String,
    /// The item's Bazaar DB id, which the shop odds page takes as `?item=`.
    pub item_url_id: String,
    /// The hero the odds are for.
    pub hero: String,
    /// The tier the item is offered at, for an owned item.
    pub tier_offered: Option<String>,
    /// The tiers the item comes in, such as `Bronze`, `Silver`, `Gold`.
    pub item_tiers: Vec<String>,
    /// The day columns: `1`, `2`, … and a last one such as `10+`.
    pub days: Vec<String>,
    /// Whether each day has enough tracked runs to be measured.
    pub measured: Vec<bool>,
    /// "Any shop": every merchant offered that day, each visited and rerolled.
    pub any_reroll_by_day: Vec<Option<f64>>,
    pub merchants: Vec<MerchantOdds>,
}

impl ItemOddsTable {
    /// Which column holds `day`: its own, or the last one when it is a `N+` column that covers it.
    pub fn column_of(&self, day: u32) -> Option<usize> {
        self.days.iter().position(|label| label.trim().parse::<u32>().ok() == Some(day)).or_else(|| {
            let last = self.days.len().checked_sub(1)?;
            let from: u32 = self.days[last].trim().strip_suffix('+')?.trim().parse().ok()?;
            (day >= from).then_some(last)
        })
    }

    /// Whether the column has enough data to show. A table without the flags counts every column.
    pub fn is_measured(&self, column: usize) -> bool {
        self.measured.get(column).copied().unwrap_or(true)
    }

    /// The chance at `merchant` (by name, as the game or Bazaar DB writes it) in `column`.
    pub fn merchant_chance(&self, merchant: &str, column: usize) -> Option<f64> {
        let wanted = name_key(merchant);
        let row = self.merchants.iter().find(|row| name_key(&row.name) == wanted)?;
        row.days.get(column)?.offer_reroll.filter(|_| self.is_measured(column))
    }

    /// The "Any shop" chance in `column`.
    pub fn any_chance(&self, column: usize) -> Option<f64> {
        self.any_reroll_by_day.get(column).copied().flatten().filter(|_| self.is_measured(column))
    }
}

/// A name with case, spaces and punctuation taken out, so `Pasta Maker` and `pasta-maker` match.
pub fn name_key(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Answer {
    item_name: Option<String>,
    item_url_id: Option<String>,
    hero: Option<String>,
    tier_offered: Option<String>,
    item_tiers: Vec<String>,
    days: Vec<Value>,
    owned_data_by_day: Vec<Value>,
    p_any_reroll_by_day: Vec<Value>,
    merchants: Vec<RawMerchant>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawMerchant {
    name: Option<String>,
    always_tier: Option<String>,
    days: Vec<RawDay>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawDay {
    p_offer_reroll: Option<f64>,
    p_seen: Option<f64>,
}

/// A day label, whether the answer wrote it as a number or as text.
fn label(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Whether a value counts as true, as the page's script treats it.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Reads the answer of `/api/item-odds`.
pub fn parse_json(text: &str) -> Result<ItemOddsTable, serde_json::Error> {
    let answer: Answer = serde_json::from_str(text.trim())?;
    Ok(ItemOddsTable {
        item_name: answer.item_name.unwrap_or_default(),
        item_url_id: answer.item_url_id.unwrap_or_default(),
        hero: answer.hero.unwrap_or_default(),
        tier_offered: answer.tier_offered,
        item_tiers: answer.item_tiers,
        days: answer.days.iter().map(label).collect(),
        measured: answer.owned_data_by_day.iter().map(truthy).collect(),
        any_reroll_by_day: answer.p_any_reroll_by_day.iter().map(Value::as_f64).collect(),
        merchants: answer
            .merchants
            .into_iter()
            .map(|raw| MerchantOdds {
                name: raw.name.unwrap_or_default(),
                always_tier: raw.always_tier,
                days: raw.days.into_iter().map(|day| MerchantDay { offer_reroll: day.p_offer_reroll, seen: day.p_seen }).collect(),
            })
            .collect(),
    })
}

#[cfg(test)]
#[path = "item_odds_tests.rs"]
mod tests;
