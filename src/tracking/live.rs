//! The Live section: what the game shows now, read about once a second while the tracker is
//! attached. Nothing here is saved; it is replaced by the next read.

use bazaar_tracker::memory::{BoardState, Item};
use bazaar_tracker::tracker::Notice;

use super::run::board::{BoardView, Grid, SkillChip, board_grid, skill_chips, stash_grid, time_of, view_of};
use super::run::detail::CardView;
use super::item_odds::Owned;
use super::names::Names;

/// The latest board read, if there is one.
#[derive(Debug, Default)]
pub struct LiveState {
    board: Option<BoardState>,
}

impl LiveState {
    /// Take a notice in: a read replaces the board, and losing the game forgets it.
    pub fn apply(&mut self, notice: &Notice) {
        use bazaar_tracker::memory::ConnectionChange;
        match notice {
            Notice::LiveSnapshot(board) => self.board = board.clone(),
            Notice::Connection(ConnectionChange::Disconnected { .. }) => self.board = None,
            _ => {}
        }
    }

    /// The hero of the last board read, if the game had one.
    pub fn hero(&self) -> Option<&str> {
        self.board.as_ref()?.player.stats.hero.as_deref()
    }

    /// What the Live section shows. `attached` says whether the game is attached and `run_live`
    /// whether a Run is being played.
    pub fn view(&self, attached: bool, run_live: bool, names: &Names) -> Live {
        match (&self.board, attached, run_live) {
            (_, false, _) => Live::Placeholder("Waiting for the game. What it shows appears here while it is attached."),
            (_, true, false) | (None, true, _) => Live::Placeholder("No run is being played right now."),
            (Some(board), true, true) => Live::View(Box::new(live_view(board, names))),
        }
    }
}

/// The Live section: a view, or why there is none.
#[derive(Debug, PartialEq, Eq)]
pub enum Live {
    Placeholder(&'static str),
    View(Box<LiveView>),
}

/// What is on offer: the shop's items or the encounters, as tiles on a row of ten slots each taking
/// its size; an id the read has no card for, by its id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offered {
    /// `SHOP`, `CHOICES` or `ON OFFER`.
    pub title: &'static str,
    pub items: Grid,
    pub cards: Vec<CardView>,
}

/// The opponent in a combat: their skills and their board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opponent {
    pub board: Grid,
    pub skills: Vec<SkillChip>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveView {
    /// `Shop · Day 2 Hour 1`: the game's state and the time in the run.
    pub state: String,
    pub offered: Option<Offered>,
    /// The player's own items as they are now.
    pub mine: BoardView,
    /// The opponent's skills and board, in a combat where either was read.
    pub opponent: Option<Opponent>,
    /// Whether the game is in a combat: the section shows the opponent over the player, and
    /// leaves the stash out.
    pub combat: bool,
    /// The hero and the day, as the game has them, for the item odds.
    pub hero: Option<String>,
    pub day: Option<i32>,
    /// The player's items on the board and in the stash, one for each card and tier.
    pub owned: Vec<Owned>,
    /// The same, on the board only and in the stash only, for the odds' filter.
    pub board_owned: Vec<Owned>,
    pub stash_owned: Vec<Owned>,
    /// Whether the player is in a merchant's shop (run state `Encounter`).
    pub in_shop: bool,
    /// At an encounter choice, the names of the cards on offer: the merchants to show odds beside.
    pub merchants: Vec<String>,
}

/// The words for a run state of the game, and what its offers are called.
fn state_words(run_state: Option<&str>) -> (String, &'static str) {
    match run_state {
        Some("Encounter") => ("Shop".to_owned(), "SHOP"),
        Some("Choice") => ("Encounter choice".to_owned(), "CHOICES"),
        Some("Combat" | "PVPCombat") => ("Combat".to_owned(), "ON OFFER"),
        Some("Loot") => ("Loot".to_owned(), "ON OFFER"),
        Some(other) => (other.to_owned(), "ON OFFER"),
        None => ("Unknown state".to_owned(), "ON OFFER"),
    }
}

/// The Live section for one board read.
pub fn live_view(board: &BoardState, names: &Names) -> LiveView {
    let (words, title) = state_words(board.run_state.as_deref());
    let stats = &board.player.stats;
    let state = match (stats.day, stats.hour) {
        (Some(day), Some(hour)) => format!("{words} · Day {day} Hour {hour}"),
        (Some(day), None) => format!("{words} · Day {day}"),
        _ => words.clone(),
    };

    let mut items: Vec<Item> = Vec::new();
    let mut cards = Vec::new();
    // Items and encounters alike are tiles of their own size, as on the board.
    for id in &board.selection_set {
        match board.entity(id) {
            Some(entity) => items.push(entity.clone()),
            None => cards.push(CardView { name: id.clone(), meta: String::new(), tier: None, picked: false }),
        }
    }
    let offered = (!items.is_empty() || !cards.is_empty()).then(|| Offered { title, items: stash_grid(&items, names), cards });

    let combat = matches!(board.run_state.as_deref(), Some("Combat" | "PVPCombat"));
    let them = &board.opponent;
    let opponent = (combat && !(them.board.is_empty() && them.skills.is_empty())).then(|| Opponent { board: board_grid(&them.board, names), skills: skill_chips(&them.skills, names) });
    // The player's section names the game's state, with the time in the run beside it.
    let mine = view_of(&board.player, words, time_of(stats.day, stats.hour), names);
    // The items owned, once per card and tier: on the board, in the stash, and both.
    let owned_of = |items: &mut dyn Iterator<Item = &Item>| {
        let mut owned: Vec<Owned> = Vec::new();
        for item in items.filter(|item| item.card_type == "Item") {
            if let Some(template_id) = &item.template_id
                && !owned.iter().any(|seen| seen.template_id.eq_ignore_ascii_case(template_id) && seen.tier == item.tier)
            {
                owned.push(Owned { template_id: template_id.clone(), tier: item.tier.clone(), name: names.card(Some(template_id), &item.instance_id) });
            }
        }
        owned
    };
    let owned = owned_of(&mut board.player.board.iter().chain(&board.player.stash));
    let board_owned = owned_of(&mut board.player.board.iter());
    let stash_owned = owned_of(&mut board.player.stash.iter());
    let merchants = if board.run_state.as_deref() == Some("Choice") { items.iter().map(|item| names.card(item.template_id.as_deref(), &item.instance_id)).collect() } else { Vec::new() };
    let in_shop = board.run_state.as_deref() == Some("Encounter");
    LiveView { state, offered, mine, opponent, combat, hero: stats.hero.clone(), day: stats.day, owned, board_owned, stash_owned, in_shop, merchants }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
