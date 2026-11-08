//! The recommendation card, from a `suggest` tool call: the ranked rows best first, each with what
//! the card shows (size, tier, whether it fits, the reason, and bars with their `n`), the source
//! runs behind each bar for the hover and expand, and the source line with its thin-data warning.
//! Plain data: the drawing is `ui/advisor.rs`, the mini and full Boards `ui/run/board.rs`.
//!
//! The result's shape is `suggest`'s in docs/documents/advisor.md ("The tools").

use std::collections::BTreeMap;

use bazaar_tracker::memory::Item;
use serde_json::Value;

use crate::tracking::names::Names;
use crate::tracking::run::board::{self, Grid, SkillChip};

/// What the source line says when the data is thin.
pub const THIN_MARK: &str = "⚠";

/// What a row with no data shows in place of its bars.
pub const NO_DATA: &str = "no data · from card text";

/// The card, as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Recommendation {
    pub title: String,
    /// Best first.
    pub rows: Vec<Row>,
    /// The source line; none while `suggest` runs.
    pub source: Option<Source>,
    /// Every run behind a bar, by id.
    pub runs: BTreeMap<String, SourceRun>,
    /// Still running: drawn from the arguments, with no numbers yet.
    pub pending: bool,
}

/// Which runs and seasons the numbers come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// `15 Jules runs · S20 · 1 profile · thin ⚠`.
    pub text: String,
    /// Under 10 runs: drawn as a warning.
    pub thin: bool,
}

/// Whether an option fits in the Board's free slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// ✓ fits
    Yes,
    /// ✕ doesn't fit
    No,
    /// A skill or encounter, or the Board or size isn't known: nothing is shown.
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub rank: usize,
    pub card: String,
    /// The card's id, for its art (`Names::art`).
    pub id: Option<String>,
    /// `Small`, `Medium`, `Large`.
    pub size: Option<String>,
    /// `Bronze`…`Legendary`.
    pub tier: Option<String>,
    pub fit: Fit,
    pub reason: String,
    pub data: RowData,
}

impl Row {
    /// Every run behind the row's bars, each once, in order: the runs that can be expanded under it.
    pub fn run_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::new();
        if let RowData::Bars(bars) = &self.data {
            for run in bars.iter().flat_map(|bar| &bar.runs) {
                if !ids.contains(&run.id.as_str()) {
                    ids.push(&run.id);
                }
            }
        }
        ids
    }
}

/// What a row shows under its reason.
#[derive(Clone, Debug, PartialEq)]
pub enum RowData {
    /// No numbers yet (`suggest` is running), or none came.
    Waiting,
    /// Never on a final Board and never offered in the sample: "no data · from card text".
    NoData,
    Bars(Vec<Bar>),
}

/// One bar: a count out of a total, with the runs behind the count.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    /// `with your Board`, `taken when offered`, `on final Boards`.
    pub label: String,
    pub n: u32,
    pub of: u32,
    /// Shown after the count: `with Pizza Cutter · lift 2.81`.
    pub note: Option<String>,
    /// The runs the hover lists, each once, in order.
    pub runs: Vec<BarRun>,
}

impl Bar {
    /// How full it is, 0 to 1 (empty with nothing to count).
    pub fn fraction(&self) -> f32 {
        if self.of == 0 { 0. } else { (self.n as f32 / self.of as f32).clamp(0., 1.) }
    }

    /// `3/4`.
    pub fn count(&self) -> String {
        format!("{}/{}", self.n, self.of)
    }
}

/// A run behind a bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarRun {
    pub id: String,
    /// On "taken when offered": the card was taken in this run.
    pub taken: bool,
}

/// One Scraped run, as the hover and the expanded run show it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceRun {
    pub id: String,
    pub hero: Option<String>,
    pub season: Option<i64>,
    pub wins: Option<i64>,
    pub losses: Option<i64>,
    /// `Gold Victory`.
    pub victory_tier: Option<String>,
    /// Its Bazaar DB page.
    pub url: Option<String>,
    /// In slot order.
    pub board: Vec<RunCard>,
    pub skills: Vec<RunCard>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunCard {
    pub id: String,
    pub name: String,
    pub tier: Option<String>,
    pub slot: Option<u8>,
    /// In slots: 1, 2 or 3.
    pub size: Option<u8>,
    pub enchantment: Option<String>,
}

impl SourceRun {
    /// `S20 · 10–3 · Gold Victory`: what is known of it.
    pub fn result(&self) -> String {
        let mut parts = Vec::new();
        parts.extend(self.season.map(|season| format!("S{season}")));
        match (self.wins, self.losses) {
            (Some(wins), Some(losses)) => parts.push(format!("{wins}–{losses}")),
            (Some(wins), None) => parts.push(format!("{wins} wins")),
            _ => {}
        }
        parts.extend(self.victory_tier.clone());
        if parts.is_empty() { "result not known".to_owned() } else { parts.join(" · ") }
    }

    /// Its page on Bazaar DB, if it has one there: only `https://bazaardb.gg/` links are opened.
    pub fn link(&self) -> Option<&str> {
        self.url.as_deref().filter(|url| url.starts_with("https://bazaardb.gg/"))
    }

    /// Its final Board as the board renderer lays it out, each item on its slot. A card the catalog
    /// doesn't know keeps the name the run gave it.
    pub fn board_grid(&self, names: &Names) -> Grid {
        let items: Vec<Item> = self.board.iter().enumerate().map(|(index, card)| card.item(index, "Item", true)).collect();
        let mut grid = board::board_grid(&items, names);
        for tile in &mut grid.tiles {
            if let Some(card) = self.board.iter().find(|card| Some(&card.id) == tile.template_id.as_ref())
                && tile.name == card.id
            {
                tile.name = card.name.clone();
            }
        }
        grid
    }

    /// Its skills as chips, rarest first.
    pub fn skill_chips(&self, names: &Names) -> Vec<SkillChip> {
        let items: Vec<Item> = self.skills.iter().enumerate().map(|(index, card)| card.item(index, "Skill", false)).collect();
        let mut chips = board::skill_chips(&items, names);
        for chip in &mut chips {
            if let Some(card) = self.skills.iter().find(|card| card.id == chip.name) {
                chip.name = card.name.clone();
            }
        }
        chips
    }
}

impl RunCard {
    /// As the game's board read would give it, for the board renderer.
    fn item(&self, index: usize, card_type: &str, on_board: bool) -> Item {
        let size = match self.size {
            Some(2) => "Medium",
            Some(3) => "Large",
            _ => "Small",
        };
        Item {
            instance_id: format!("run-{index}"),
            template_id: Some(self.id.clone()),
            card_type: card_type.to_owned(),
            tier: self.tier.clone().unwrap_or_default(),
            size: size.to_owned(),
            socket: if on_board { self.slot.map(|slot| format!("Socket_{slot}")) } else { None },
            enchantment: self.enchantment.clone(),
            ..Item::default()
        }
    }
}

/// Which count's popup is showing. The popup stays while the pointer is over the count or over
/// the popup, so it can move from one to the other and click a run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hover {
    count: Option<String>,
    popup: Option<String>,
}

impl Hover {
    /// The pointer came onto (or left) the count with this key.
    pub fn count(&mut self, key: &str, on: bool) {
        Self::set(&mut self.count, key, on);
    }

    /// The pointer came onto (or left) the popup of the count with this key.
    pub fn popup(&mut self, key: &str, on: bool) {
        Self::set(&mut self.popup, key, on);
    }

    fn set(slot: &mut Option<String>, key: &str, on: bool) {
        if on {
            *slot = Some(key.to_owned());
        } else if slot.as_deref() == Some(key) {
            *slot = None;
        }
    }

    /// The count whose popup shows: the one under the pointer, else the one whose popup is.
    pub fn shown(&self) -> Option<&str> {
        self.count.as_deref().or(self.popup.as_deref())
    }
}

impl Recommendation {
    /// The card from `suggest`'s arguments and, once it has run without error, its result. With no
    /// result yet it is pending, drawn from the arguments.
    pub fn of(arguments: &Value, result: Option<&Value>, pending: bool) -> Option<Self> {
        let done = result.filter(|output| output.get("ranked").is_some());
        let from = done.unwrap_or(arguments);
        let mut rows: Vec<Row> = from.get("ranked")?.as_array()?.iter().enumerate().filter_map(|(index, row)| row_of(index, row, pending)).collect();
        rows.sort_by_key(|row| row.rank);
        let runs = done
            .and_then(|output| output.get("runs"))
            .and_then(Value::as_object)
            .map(|runs| runs.iter().map(|(id, run)| (id.clone(), run_of(id, run))).collect())
            .unwrap_or_default();
        Some(Self { title: text(from, "title").unwrap_or_default(), rows, source: done.map(source_of), runs, pending })
    }

    /// The runs a bar's hover lists that the result carries, with whether each was taken.
    pub fn runs_of<'a>(&'a self, bar: &'a Bar) -> impl Iterator<Item = (&'a SourceRun, bool)> + 'a {
        bar.runs.iter().filter_map(|run| self.runs.get(&run.id).map(|found| (found, run.taken)))
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty()).map(str::to_owned)
}

fn count(value: &Value, key: &str) -> u32 {
    value.get(key).and_then(Value::as_u64).map_or(0, |n| n.min(u64::from(u32::MAX)) as u32)
}

/// Ids in order, each once.
fn ids(value: Option<&Value>) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in value.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        if !ids.iter().any(|seen| seen == id) {
            ids.push(id.to_owned());
        }
    }
    ids
}

/// `small` or `SMALL` as `Small`.
fn capitalised(text: String) -> String {
    let lower = text.to_lowercase();
    let mut chars = lower.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn row_of(index: usize, row: &Value, pending: bool) -> Option<Row> {
    let rank = row.get("rank").and_then(Value::as_u64).map_or(index + 1, |rank| rank as usize);
    let fit = match row.get("fits").and_then(Value::as_bool) {
        Some(true) => Fit::Yes,
        Some(false) => Fit::No,
        None => Fit::Unknown,
    };
    let data = if pending || (row.get("runs_with").is_none() && row.get("picks").is_none() && row.get("no_data").is_none()) {
        RowData::Waiting
    } else if row.get("no_data").and_then(Value::as_bool).unwrap_or(false) {
        RowData::NoData
    } else {
        RowData::Bars(bars(row))
    };
    Some(Row {
        rank,
        card: text(row, "card")?,
        id: text(row, "id"),
        size: text(row, "size").map(capitalised),
        tier: text(row, "tier").map(capitalised),
        fit,
        reason: text(row, "reason").unwrap_or_default(),
        data,
    })
}

/// "with your Board" (the Board card it goes with best: the highest lift, then the most runs),
/// else "on final Boards" when the Board is empty; then "taken when offered".
fn bars(row: &Value) -> Vec<Bar> {
    let mut bars = Vec::new();
    let with_board: Vec<&Value> = row.get("with_board").and_then(Value::as_array).map(|fits| fits.iter().collect()).unwrap_or_default();
    let lift = |fit: &Value| fit.get("lift").and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY);
    let best = with_board.iter().copied().max_by(|a, b| lift(a).total_cmp(&lift(b)).then_with(|| count(a, "n").cmp(&count(b, "n"))));
    match best {
        Some(fit) => {
            let mut note = text(fit, "card").map(|card| format!("with {card}"));
            if let Some(value) = fit.get("lift").and_then(Value::as_f64) {
                let lift = format!("lift {value:.2}");
                note = Some(note.map_or(lift.clone(), |note| format!("{note} · {lift}")));
            }
            let runs = ids(fit.get("run_ids")).into_iter().map(|id| BarRun { id, taken: false }).collect();
            bars.push(Bar { label: "with your Board".into(), n: count(fit, "n"), of: count(fit, "of"), note, runs });
        }
        None => {
            if let Some(runs_with) = row.get("runs_with") {
                let runs = ids(runs_with.get("run_ids")).into_iter().map(|id| BarRun { id, taken: false }).collect();
                bars.push(Bar { label: "on final Boards".into(), n: count(runs_with, "n"), of: count(runs_with, "of"), note: None, runs });
            }
        }
    }
    if let Some(picks) = row.get("picks") {
        let taken = ids(picks.get("taken_run_ids"));
        let runs = ids(picks.get("offered_run_ids")).into_iter().map(|id| BarRun { taken: taken.contains(&id), id }).collect();
        bars.push(Bar { label: "taken when offered".into(), n: count(picks, "taken"), of: count(picks, "offered"), note: None, runs });
    }
    bars
}

/// The source line: the result's `source`, else one made from its `n`, hero and sample; with ⚠
/// thin under 10 runs (the result's `thin`, else its `n` under 10).
fn source_of(output: &Value) -> Source {
    let n = count(output, "n");
    let thin = output.get("thin").and_then(Value::as_bool).unwrap_or(output.get("n").is_some() && n < 10);
    let mut text = text(output, "source").unwrap_or_else(|| {
        let hero = text(output, "hero").map(|hero| format!(" {hero}")).unwrap_or_default();
        let mut line = format!("{n}{hero} run{}", if n == 1 { "" } else { "s" });
        if let Some(sample) = text(output, "sample").filter(|_| n > 0) {
            line += &format!(" · {sample}");
        }
        if let Some(profiles) = output.get("profiles").and_then(Value::as_u64) {
            line += &format!(" · {profiles} profile{}", if profiles == 1 { "" } else { "s" });
        }
        line
    });
    if thin && !text.contains(THIN_MARK) {
        text += &format!(" · thin {THIN_MARK}");
    }
    Source { text, thin }
}

fn run_of(id: &str, run: &Value) -> SourceRun {
    let cards = |key: &str| -> Vec<RunCard> {
        run.get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|card| {
                let id = text(card, "id")?;
                Some(RunCard {
                    name: text(card, "name").unwrap_or_else(|| id.clone()),
                    tier: text(card, "tier"),
                    slot: card.get("slot").and_then(Value::as_u64).and_then(|slot| u8::try_from(slot).ok()),
                    size: card.get("size").and_then(Value::as_u64).and_then(|size| u8::try_from(size).ok()),
                    enchantment: text(card, "enchantment"),
                    id,
                })
            })
            .collect()
    };
    SourceRun {
        id: text(run, "run_id").unwrap_or_else(|| id.to_owned()),
        hero: text(run, "hero"),
        season: run.get("season").and_then(Value::as_i64),
        wins: run.get("wins").and_then(Value::as_i64),
        losses: run.get("losses").and_then(Value::as_i64),
        victory_tier: text(run, "victory_tier"),
        url: text(run, "url"),
        board: cards("board"),
        skills: cards("skills"),
    }
}

#[cfg(test)]
#[path = "recommendation_tests.rs"]
mod tests;
