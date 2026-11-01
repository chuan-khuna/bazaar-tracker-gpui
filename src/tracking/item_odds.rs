//! Bazaar DB's odds for single items: the ones saved in the history, the item being loaded, the
//! pop-up's table, and the chances beside the merchants of an encounter choice.
//!
//! Nothing is loaded on its own. The player asks for one item from its tile; the shop odds page in
//! the web view asks Bazaar DB for it (the player picks the item there the first time, after which
//! its Bazaar DB id is known), and the reply is saved only if it is for the item that was asked for.
//! Odds are kept for the item, the hero and the tier the player owns it at.

use std::collections::HashMap;

use bazaar_tracker::history::SavedItemOdds;
use bazaardb::ItemOddsTable;
use bazaardb::item_odds::name_key;

use super::when;

/// The tier asked about for an item the player does not own.
pub const NOT_OWNED: &str = "None";

/// One item's saved odds, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub saved: SavedItemOdds,
    pub table: ItemOddsTable,
}

/// Every item's saved odds, by template id (lower-cased), hero and tier.
#[derive(Debug, Default)]
pub struct ItemOddsBook {
    entries: HashMap<(String, String, String), Entry>,
}

impl ItemOddsBook {
    /// The odds in the history. One whose answer can not be read is left out and logged.
    pub fn load(saved: Vec<SavedItemOdds>) -> Self {
        let mut book = Self::default();
        for odds in saved {
            if let Err(why) = book.insert(odds) {
                tracing::warn!("item odds: {why}");
            }
        }
        book
    }

    /// Keep `saved`, replacing any for the same item, hero and tier.
    pub fn insert(&mut self, saved: SavedItemOdds) -> Result<(), String> {
        let table = bazaardb::item_odds::parse_json(&saved.body).map_err(|error| format!("the saved odds for {} can not be read: {error}", saved.item_name))?;
        self.entries.insert((saved.template_id.to_lowercase(), saved.hero.clone(), saved.tier.clone()), Entry { saved, table });
        Ok(())
    }

    pub fn get(&self, template_id: &str, hero: &str, tier: &str) -> Option<&Entry> {
        self.entries.get(&(template_id.to_lowercase(), hero.to_owned(), tier.to_owned()))
    }

    /// The item's Bazaar DB id, from any odds saved for it.
    pub fn url_id(&self, template_id: &str) -> Option<&str> {
        let template_id = template_id.to_lowercase();
        self.entries.iter().find(|((id, _, _), entry)| *id == template_id && !entry.saved.url_id.is_empty()).map(|(_, entry)| entry.saved.url_id.as_str())
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The item the player asked odds for: the game's id and name for it, the hero and the tier owned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub template_id: String,
    pub name: String,
    pub hero: String,
    pub tier: String,
}

/// A reply that was for the item asked about, ready to save.
#[derive(Debug, PartialEq)]
pub struct Accepted {
    pub saved: SavedItemOdds,
    /// Something the player should know, such as odds saved for another hero than the Run's.
    pub note: Option<String>,
}

/// Whether `reply` (the request's item, hero and tier, and Bazaar DB's answer) is for `wanted`: its
/// name matches, or its Bazaar DB id is the one already known for the item (`known_url_id`). The
/// odds are saved under the hero and tier they are for, which the player may have changed on the page.
pub fn accept(wanted: &Wanted, reply: &bazaardb::ItemOdds, known_url_id: Option<&str>, now_ms: i64) -> Result<Accepted, String> {
    let table = reply.table().map_err(|error| format!("Bazaar DB's answer could not be read: {error}"))?;
    let url_id = if table.item_url_id.is_empty() { reply.item.clone() } else { table.item_url_id.clone() };
    let same_id = known_url_id.is_some_and(|known| known == url_id || known == reply.item);
    if !same_id && name_key(&table.item_name) != name_key(&wanted.name) {
        let got = if table.item_name.is_empty() { "another item".to_owned() } else { table.item_name.clone() };
        return Err(format!("Bazaar DB sent the odds for {got}, not {}; they were not saved.", wanted.name));
    }
    let hero = if table.hero.is_empty() { reply.hero.clone() } else { table.hero.clone() };
    let tier = if reply.tier.is_empty() { NOT_OWNED.to_owned() } else { reply.tier.clone() };
    let mut notes = Vec::new();
    if hero != wanted.hero {
        notes.push(format!("they are for {hero}, not {}", wanted.hero));
    }
    if tier != wanted.tier {
        notes.push(format!("they are for {} owned, not {}", tier_words(&tier), tier_words(&wanted.tier)));
    }
    let note = (!notes.is_empty()).then(|| format!("Saved the odds for {}, but {}.", wanted.name, notes.join(" and ")));
    let item_name = if table.item_name.is_empty() { wanted.name.clone() } else { table.item_name.clone() };
    Ok(Accepted { saved: SavedItemOdds { template_id: wanted.template_id.to_lowercase(), hero, tier, url_id, item_name, body: reply.body.clone(), saved_ms: now_ms }, note })
}

/// `Silver`, or `not owned`.
fn tier_words(tier: &str) -> String {
    if tier == NOT_OWNED { "not owned".to_owned() } else { tier.to_owned() }
}

/// A chance from 0 to 1 as the page prints it: `12.5%`, or `0.25%` under ten percent.
pub fn percent(chance: f64) -> String {
    let value = 100. * chance;
    let text = if chance >= 0.1 { format!("{value:.1}") } else { format!("{value:.2}") };
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}%")
}

/// One row of the pop-up's table.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub name: String,
    /// Under it, such as `always Silver`.
    pub note: Option<String>,
    /// One cell for each day; `None` where there is too little data.
    pub cells: Vec<Option<String>>,
}

/// The pop-up for one item's odds.
#[derive(Clone, Debug, PartialEq)]
pub struct TableView {
    pub title: String,
    /// `Jules · owned Bronze · offered at Silver`.
    pub facts: String,
    /// `Saved 1 Nov 2026 20:40`.
    pub saved: String,
    /// `Day 1`, `2`, … `10+`.
    pub columns: Vec<String>,
    /// The column of the day being looked at.
    pub today: Option<usize>,
    pub rows: Vec<TableRow>,
}

/// The pop-up for `entry`, with the column of `day` marked.
pub fn table_view(entry: &Entry, day: Option<u32>) -> TableView {
    let table = &entry.table;
    let mut facts = vec![entry.saved.hero.clone(), if entry.saved.tier == NOT_OWNED { "not owned".to_owned() } else { format!("owned {}", entry.saved.tier) }];
    facts.extend(table.tier_offered.as_ref().map(|tier| format!("offered at {tier}")));
    let columns = table.days.iter().enumerate().map(|(index, label)| if index == 0 { format!("Day {label}") } else { label.clone() }).collect();
    let cell = |column: usize, chance: Option<f64>| chance.filter(|_| table.is_measured(column)).map(percent);
    let mut rows = Vec::new();
    if table.any_reroll_by_day.iter().any(Option::is_some) {
        rows.push(TableRow { name: "Any shop".into(), note: Some("presented as an option that day".into()), cells: (0..table.days.len()).map(|column| cell(column, table.any_reroll_by_day.get(column).copied().flatten())).collect() });
    }
    rows.extend(table.merchants.iter().map(|merchant| TableRow {
        name: merchant.name.clone(),
        note: merchant.always_tier.as_ref().map(|tier| format!("always {tier}")),
        cells: (0..table.days.len()).map(|column| cell(column, merchant.days.get(column).and_then(|day| day.offer_reroll))).collect(),
    }));
    TableView {
        title: entry.saved.item_name.clone(),
        facts: facts.join(" · "),
        saved: format!("Saved {}", when::stamp(entry.saved.saved_ms)),
        columns,
        today: day.and_then(|day| table.column_of(day)),
        rows,
    }
}

/// An item the player owns, as the Live section knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    pub template_id: String,
    pub tier: String,
    pub name: String,
}

/// The chances beside one merchant of an encounter choice (or "Any shop").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerchantLine {
    pub merchant: String,
    /// Each owned item with saved odds, and its chance there today.
    pub items: Vec<(String, String)>,
}

/// At an encounter choice on `day`, the chance each item in `owned` that has saved odds for `hero`
/// is in stock at each of `merchants`: first "Any shop", then each merchant Bazaar DB knows by that
/// name. A merchant with no chances is left out.
pub fn choice_lines(book: &ItemOddsBook, hero: &str, owned: &[Owned], merchants: &[String], day: u32) -> Vec<MerchantLine> {
    let entries: Vec<(&Owned, &Entry)> = owned.iter().filter_map(|item| Some((item, book.get(&item.template_id, hero, &item.tier)?))).collect();
    if entries.is_empty() {
        return Vec::new();
    }
    let line = |merchant: String, chance: &dyn Fn(&ItemOddsTable, usize) -> Option<f64>| {
        let items: Vec<(String, String)> = entries
            .iter()
            .filter_map(|(item, entry)| {
                let column = entry.table.column_of(day)?;
                Some((item.name.clone(), percent(chance(&entry.table, column)?)))
            })
            .collect();
        (!items.is_empty()).then_some(MerchantLine { merchant, items })
    };
    let mut lines: Vec<MerchantLine> = line("Any shop".to_owned(), &|table, column| table.any_chance(column)).into_iter().collect();
    for merchant in merchants {
        lines.extend(line(merchant.clone(), &|table, column| table.merchant_chance(merchant, column)));
    }
    lines
}

#[cfg(test)]
#[path = "item_odds_tests.rs"]
mod tests;
