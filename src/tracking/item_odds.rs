//! Bazaar DB's odds for single items: the ones saved in the history, the item being loaded, the
//! pop-up's table, and the chances at the merchants of an encounter choice or of the shop the
//! player is in.
//!
//! The player asks for one item from its tile, or the watched items' missing odds load in the
//! background (`ui::odds_load`). The shop odds page in the web view asks Bazaar DB for it (the item
//! is found in the page's search the first time, after which its Bazaar DB id is known), and the
//! reply is saved only if it is for the item that was asked for.
//! Odds are kept for the item, the hero and the tier the player owns it at.

use std::collections::HashMap;

use bazaar_tracker::catalog::NEUTRAL_HERO;
use bazaar_tracker::enums::hero_names;
use bazaar_tracker::history::SavedItemOdds;
use bazaardb::ItemOddsTable;
use bazaardb::item_odds::name_key;

use super::names::Names;
use super::when;
use bazaar_tracker::run::{Event, EventKind};

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
    let mut facts = vec![bazaar_tracker::enums::hero_names::display_name(&entry.saved.hero).to_owned(), if entry.saved.tier == NOT_OWNED { "not owned".to_owned() } else { format!("owned {}", entry.saved.tier) }];
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

/// One merchant of an encounter choice (or "Any shop"), with the chance of each watched item there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerchantGroup {
    pub merchant: String,
    /// Each watched item with saved odds, and its chance there today.
    pub items: Vec<Chance>,
}

/// A watched item's chance at one merchant, as the page prints it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chance {
    pub template_id: String,
    pub name: String,
    /// The tier owned, as its first letter, when the item is owned at more than one tier.
    pub tier_mark: Option<String>,
    pub chance: String,
}

/// What the player wants to see beside the merchants: the items they own, at the tier owned, and
/// the items they track, as not owned unless they own them too, less the ones `hidden`. One for
/// each card and tier owned.
pub fn watched(owned: &[Owned], tracked: &[(String, String)], hidden: &[String]) -> Vec<Owned> {
    let mut watched: Vec<Owned> = Vec::new();
    for item in owned {
        if !watched.iter().any(|seen| seen.template_id.eq_ignore_ascii_case(&item.template_id) && seen.tier == item.tier) {
            watched.push(item.clone());
        }
    }
    for (template_id, name) in tracked {
        if !watched.iter().any(|seen| seen.template_id.eq_ignore_ascii_case(template_id)) {
            watched.push(Owned { template_id: template_id.clone(), tier: NOT_OWNED.to_owned(), name: name.clone() });
        }
    }
    watched.retain(|item| !hidden.iter().any(|id| id.eq_ignore_ascii_case(&item.template_id)));
    watched
}

/// At an encounter choice on `day`, the chance each `watched` item with saved odds for `hero` is in
/// stock at each of `merchants`, then, with `any_shop`, at any shop, last. A merchant Bazaar DB
/// lists none of them at has an empty group, so the player sees it carries none; Any shop with no
/// chances is left out. Nothing at all when no watched item has saved odds.
pub fn choice_groups(book: &ItemOddsBook, hero: &str, watched: &[Owned], merchants: &[String], day: u32, any_shop: bool) -> Vec<MerchantGroup> {
    let entries: Vec<(&Owned, &Entry)> = watched.iter().filter_map(|item| Some((item, book.get(&item.template_id, hero, &item.tier)?))).collect();
    if entries.is_empty() {
        return Vec::new();
    }
    // An item owned at more than one tier shows each, marked with its tier.
    let tier_mark = |item: &Owned| {
        let tiers = entries.iter().filter(|(other, _)| other.template_id.eq_ignore_ascii_case(&item.template_id)).count();
        (tiers > 1).then(|| item.tier.chars().take(1).collect::<String>())
    };
    let group = |merchant: String, chance: &dyn Fn(&ItemOddsTable, usize) -> Option<f64>| {
        let items: Vec<Chance> = entries
            .iter()
            .filter_map(|(item, entry)| {
                let column = entry.table.column_of(day)?;
                Some(Chance { template_id: item.template_id.to_lowercase(), name: item.name.clone(), tier_mark: tier_mark(item), chance: percent(chance(&entry.table, column)?) })
            })
            .collect();
        MerchantGroup { merchant, items }
    };
    // Every merchant on offer has a group, an empty one when it carries none of the watched items.
    let mut groups: Vec<MerchantGroup> = merchants.iter().map(|merchant| group(merchant.clone(), &|table, column| table.merchant_chance(merchant, column))).collect();
    if any_shop {
        groups.extend(Some(group("Any shop".to_owned(), &|table, column| table.any_chance(column))).filter(|group| !group.items.is_empty()));
    }
    groups
}

/// The merchant whose shop the player is in: the encounter they chose last, by name. Nothing
/// before the first encounter of the Run.
pub fn current_merchant(events: &[Event], names: &Names) -> Option<String> {
    events.iter().rev().find_map(|event| match &event.kind {
        EventKind::Selected { card_kind, instance_id, template_id, .. } if card_kind.eq_ignore_ascii_case("Encounter") => Some(names.card(template_id.as_deref(), instance_id)),
        _ => None,
    })
}

/// A watched item and the merchants most likely to have it in stock today, best first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BestShops {
    pub template_id: String,
    pub name: String,
    /// The tier owned, as its first letter, when the item is owned at more than one tier.
    pub tier_mark: Option<String>,
    /// Each merchant's name and its chance, as the page prints it.
    pub shops: Vec<(String, String)>,
}

/// On `day`, for each `watched` item with saved odds for `hero`, the `count` merchants with the
/// best chance of having it in stock, from every merchant Bazaar DB measured that day. An item no
/// merchant has a chance for is left out.
pub fn best_shops(book: &ItemOddsBook, hero: &str, watched: &[Owned], day: u32, count: usize) -> Vec<BestShops> {
    let entries: Vec<(&Owned, &Entry)> = watched.iter().filter_map(|item| Some((item, book.get(&item.template_id, hero, &item.tier)?))).collect();
    entries
        .iter()
        .filter_map(|(item, entry)| {
            let table = &entry.table;
            let column = table.column_of(day)?;
            let mut shops: Vec<(&str, f64)> = table.merchants.iter().filter_map(|merchant| Some((merchant.name.as_str(), table.merchant_chance(&merchant.name, column)?))).collect();
            shops.sort_by(|a, b| b.1.total_cmp(&a.1));
            shops.truncate(count);
            let tiers = entries.iter().filter(|(other, _)| other.template_id.eq_ignore_ascii_case(&item.template_id)).count();
            (!shops.is_empty()).then(|| BestShops {
                template_id: item.template_id.to_lowercase(),
                name: item.name.clone(),
                tier_mark: (tiers > 1).then(|| item.tier.chars().take(1).collect()),
                shops: shops.into_iter().map(|(name, chance)| (name.to_owned(), percent(chance))).collect(),
            })
        })
        .collect()
}

/// The watched items' odds as a table: a row for each item, a column for each merchant (or for
/// each place among the best shops).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OddsGrid {
    pub columns: Vec<String>,
    pub rows: Vec<GridRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridRow {
    pub template_id: String,
    pub name: String,
    /// The tier owned, as its first letter, when the item is owned at more than one tier.
    pub tier_mark: Option<String>,
    /// One for each column; `None` where the merchant does not carry the item.
    pub cells: Vec<Option<GridCell>>,
}

/// A cell: the chance, after the merchant's name when the column does not say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridCell {
    pub merchant: Option<String>,
    pub chance: String,
}

/// An encounter choice's [`MerchantGroup`]s turned around: a column for each merchant, a row for
/// each item any of them carries, in the order first met.
pub fn choice_grid(groups: &[MerchantGroup]) -> OddsGrid {
    let mut rows: Vec<GridRow> = Vec::new();
    for (column, group) in groups.iter().enumerate() {
        for chance in &group.items {
            let index = match rows.iter().position(|row| row.template_id == chance.template_id && row.tier_mark == chance.tier_mark) {
                Some(index) => index,
                None => {
                    rows.push(GridRow { template_id: chance.template_id.clone(), name: chance.name.clone(), tier_mark: chance.tier_mark.clone(), cells: vec![None; groups.len()] });
                    rows.len() - 1
                }
            };
            rows[index].cells[column] = Some(GridCell { merchant: None, chance: chance.chance.clone() });
        }
    }
    OddsGrid { columns: groups.iter().map(|group| group.merchant.clone()).collect(), rows }
}

/// The [`BestShops`] as a table: a column for each place, `1st` to the `count`th, each cell the
/// merchant and its chance.
pub fn best_grid(best: &[BestShops], count: usize) -> OddsGrid {
    let place = |n: usize| match n {
        1 => "1st".to_owned(),
        2 => "2nd".to_owned(),
        3 => "3rd".to_owned(),
        n => format!("{n}th"),
    };
    OddsGrid {
        columns: (1..=count).map(place).collect(),
        rows: best
            .iter()
            .map(|item| GridRow {
                template_id: item.template_id.clone(),
                name: item.name.clone(),
                tier_mark: item.tier_mark.clone(),
                cells: (0..count).map(|index| item.shops.get(index).map(|(merchant, chance)| GridCell { merchant: Some(merchant.clone()), chance: chance.clone() })).collect(),
            })
            .collect(),
    }
}

/// One row of the Tracked items pop-up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub template_id: String,
    pub name: String,
    pub tracked: bool,
    /// Hidden from the odds beside the merchants; picking the row shows it again.
    pub hidden: bool,
}

/// The rows of the Tracked items pop-up: the tracked items first, in the order they were added
/// (named by `items`, else by id), then the hidden ones (a tracked item that is hidden is listed
/// there), then up to `limit` other items whose name contains `query` (ignoring case, spaces and
/// punctuation). Without a query only the tracked and hidden ones are listed.
pub fn candidates(items: &[(String, String)], tracked: &[String], hidden: &[String], query: &str, limit: usize) -> Vec<Candidate> {
    let name_of = |id: &str| items.iter().find(|(item, _)| item.eq_ignore_ascii_case(id)).map_or_else(|| id.to_owned(), |(_, name)| name.clone());
    let is_in = |ids: &[String], id: &str| ids.iter().any(|other| other.eq_ignore_ascii_case(id));
    let mut rows: Vec<Candidate> = tracked.iter().filter(|id| !is_in(hidden, id)).map(|id| Candidate { template_id: id.clone(), name: name_of(id), tracked: true, hidden: false }).collect();
    rows.extend(hidden.iter().map(|id| Candidate { template_id: id.clone(), name: name_of(id), tracked: is_in(tracked, id), hidden: true }));
    let wanted = name_key(query);
    if !wanted.is_empty() {
        let found = items
            .iter()
            .filter(|(id, name)| name_key(name).contains(&wanted) && !is_in(tracked, id) && !is_in(hidden, id))
            .take(limit)
            .map(|(id, name)| Candidate { template_id: id.clone(), name: name.clone(), tracked: false, hidden: false });
        rows.extend(found);
    }
    rows
}

/// An item of the catalog, with the heroes whose shops sell it ([`NEUTRAL_HERO`] for every hero).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogItem {
    pub template_id: String,
    pub name: String,
    pub heroes: Vec<String>,
}

/// The heroes the catalog's items name, sorted, without the neutral marker.
pub fn heroes(items: &[CatalogItem]) -> Vec<String> {
    let mut heroes: Vec<String> = items.iter().flat_map(|item| item.heroes.iter()).filter(|hero| *hero != NEUTRAL_HERO).cloned().collect();
    heroes.sort();
    heroes.dedup();
    heroes
}

/// The heroes, of those the catalog names and the one being played, that the hero list does not
/// know (see `hero_names`), sorted and once each. They are shown by the game's name.
pub fn unknown_heroes<'a>(catalog_heroes: &'a [String], playing: Option<&'a str>) -> Vec<String> {
    let mut unknown: Vec<String> = catalog_heroes.iter().map(String::as_str).chain(playing).filter(|hero| !hero_names::is_known(hero)).map(str::to_owned).collect();
    unknown.sort();
    unknown.dedup();
    unknown
}

/// The items to load odds for, as not owned, for each of `heroes` in turn: the hero's own items,
/// then the neutral ones if `neutral`. Items whose odds are saved for that hero are left out.
pub fn load_queue(items: &[CatalogItem], heroes: &[String], neutral: bool, book: &ItemOddsBook) -> Vec<Wanted> {
    let mut queue = Vec::new();
    for hero in heroes {
        let own = items.iter().filter(|item| item.heroes.contains(hero));
        let common = items.iter().filter(|item| neutral && item.heroes.iter().any(|item_hero| item_hero == NEUTRAL_HERO) && !item.heroes.contains(hero));
        for item in own.chain(common) {
            if book.get(&item.template_id, hero, NOT_OWNED).is_none() {
                queue.push(Wanted { template_id: item.template_id.clone(), name: item.name.clone(), hero: hero.clone(), tier: NOT_OWNED.to_owned() });
            }
        }
    }
    queue
}

#[cfg(test)]
#[path = "item_odds_tests.rs"]
mod tests;
