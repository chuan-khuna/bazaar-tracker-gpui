//! The Live section: what the game shows now, read about once a second while the tracker is
//! attached. Nothing here is saved; it is replaced by the next read.

use bazaar_tracker::memory::{BoardState, Item};
use bazaar_tracker::tracker::Notice;

use super::board::{BoardView, Grid, board_grid, stash_grid, view_of};
use super::detail::CardView;
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

/// What is on offer: the shop's items as tiles, and anything else (encounters) as cards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offered {
    /// `SHOP`, `CHOICES` or `ON OFFER`.
    pub title: &'static str,
    pub items: Grid,
    pub cards: Vec<CardView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveView {
    /// `Shop · Day 2 Hour 1`: the game's state and the time in the run.
    pub state: String,
    pub offered: Option<Offered>,
    /// The player's own items as they are now.
    pub mine: BoardView,
    /// The opponent's board, in a combat where it was read.
    pub opponent: Option<Grid>,
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
        _ => words,
    };

    let mut items: Vec<Item> = Vec::new();
    let mut cards = Vec::new();
    for id in &board.selection_set {
        match board.entity(id) {
            Some(entity) if id.starts_with("itm_") => items.push(entity.clone()),
            Some(entity) => {
                let meta = [entity.card_type.as_str(), entity.tier.as_str()].into_iter().filter(|text| !text.is_empty()).collect::<Vec<_>>().join(" · ");
                cards.push(CardView { name: names.card(entity.template_id.as_deref(), id), meta, tier: super::board::Tier::parse(&entity.tier), picked: false });
            }
            None => cards.push(CardView { name: id.clone(), meta: String::new(), tier: None, picked: false }),
        }
    }
    let offered = (!items.is_empty() || !cards.is_empty()).then(|| Offered { title, items: stash_grid(&items, names), cards });

    let combat = matches!(board.run_state.as_deref(), Some("Combat" | "PVPCombat"));
    LiveView {
        state: state.clone(),
        offered,
        mine: view_of(&board.player, state, names),
        opponent: (combat && !board.opponent.board.is_empty()).then(|| board_grid(&board.opponent.board, names)),
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
