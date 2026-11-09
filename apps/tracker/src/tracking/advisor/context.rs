//! The Context row under each question: the situation the app sent with it (saved with the message
//! by the backend), as the tab draws it. Collapsed, a summary line (`Context · D14 H6 · 10/10
//! slots`); opened, the player's numbers, the Board and Stash laid out as the board renderer lays out
//! a Snapshot, the skills as chips, the Offer with each card's kind, the opponent, and the raw JSON.
//! Plain data: the drawing is `ui/advisor.rs`.
//!
//! The situation is read leniently: it is what the app sent, checked by the backend, but an older
//! app may have sent less. A field left out is not shown; a list that doesn't parse is left out.

use bazaar_tracker::memory::Item;
use serde::Deserialize;
use serde_json::Value;

use super::conversation::time_label;
use super::reply::pretty;
use crate::tracking::names::Names;
use crate::tracking::run::board::{self, Grid, SLOTS, SkillChip, Tier};

/// The Context row of one question.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextView {
    /// `Context · Event #7 · D14 H6 · 10/10 slots`: the attached Event, the Day and Hour and how
    /// full the Board is, each when known.
    pub summary: String,
    /// What was sent; `None`: no situation was sent ("No context sent").
    pub sent: Option<Sent>,
}

/// A situation, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    /// Event (then Picked or Sold), Hero, Time, Gold, Level, Health, State: those the situation
    /// has, in that order.
    pub facts: Vec<(&'static str, String)>,
    /// `None`: not sent (not known); an empty grid: empty.
    pub board: Option<Grid>,
    pub stash: Option<Grid>,
    pub skills: Option<Vec<SkillChip>>,
    pub offer: Option<Vec<OfferCard>>,
    pub opponent: Option<OpponentView>,
    /// The situation exactly as sent, pretty-printed.
    pub json: String,
}

/// A card on offer, with what kind of card it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferCard {
    /// With its enchantment first, when it has one.
    pub name: String,
    pub tier: Option<Tier>,
    /// `loot`, `merchant item`, `skill`, `encounter choice`; empty when not sent.
    pub kind: String,
    pub art: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpponentView {
    /// Hero and Health, when known.
    pub facts: Vec<(&'static str, String)>,
    pub board: Grid,
    pub skills: Vec<SkillChip>,
}

/// One card as the situation has it (`agents/advisor/situation.py`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct SentCard {
    id: Option<String>,
    name: String,
    tier: Option<String>,
    size: Option<String>,
    slot: Option<u8>,
    enchantment: Option<String>,
    kind: Option<String>,
}

/// A slot effect as the situation has it: a Stove, Cooler or Note on one Board slot.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct SentSlotEffect {
    slot: u8,
    name: String,
}

/// The Event the question was sent about.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct SentEvent {
    number: Option<u64>,
    kind: String,
    time: Option<String>,
    detail: Option<String>,
    picked: Option<SentCard>,
    sold: Option<SentCard>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct SentOpponent {
    hero: Option<String>,
    health: Option<i64>,
    health_max: Option<i64>,
    board: Vec<SentCard>,
    skills: Vec<SentCard>,
    slot_effects: Vec<SentSlotEffect>,
}

/// The Context row of a question sent with `situation` (`None`, or JSON `null`: none was sent).
pub fn view(situation: Option<&Value>, names: &Names) -> ContextView {
    let Some(situation) = situation.filter(|situation| situation.is_object()) else {
        return ContextView { summary: "Context · none sent".to_owned(), sent: None };
    };
    let number = |key: &str| situation.get(key).and_then(Value::as_i64);
    let text = |key: &str| situation.get(key).and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_owned);
    let cards = |key: &str| situation.get(key).and_then(|cards| serde_json::from_value::<Vec<SentCard>>(cards.clone()).ok());

    let time = time_label(number("day").map(|day| day as i32), number("hour").map(|hour| hour as i32));
    let effects = situation.get("slot_effects").and_then(|effects| serde_json::from_value::<Vec<SentSlotEffect>>(effects.clone()).ok()).unwrap_or_default();
    let board = cards("board").map(|cards| board_grid(&cards, &effects, names));

    let event = situation.get("event").and_then(|event| serde_json::from_value::<SentEvent>(event.clone()).ok());

    let mut summary = vec!["Context".to_owned()];
    summary.extend(event.as_ref().and_then(|event| event.number).map(|number| format!("Event #{number}")));
    summary.extend(time.clone());
    summary.extend(board.as_ref().map(|grid| format!("{}/{SLOTS} slots", grid.used_slots())));

    let mut facts = Vec::new();
    if let Some(event) = &event {
        // `#7 Options: 3 options · Choice (+12:05)`.
        let mut line: Vec<String> = event.number.map(|number| format!("#{number}")).into_iter().collect();
        line.push(match event.detail.as_deref().filter(|detail| !detail.is_empty()) {
            Some(detail) => format!("{}: {detail}", event.kind),
            None => event.kind.clone(),
        });
        line.extend(event.time.as_ref().map(|time| format!("({time})")));
        facts.push(("Event", line.join(" ")));
        facts.extend(event.picked.as_ref().map(|card| ("Picked", card_name(card, names))));
        facts.extend(event.sold.as_ref().map(|card| ("Sold", card_name(card, names))));
    }
    facts.extend(text("hero").map(|hero| ("Hero", hero)));
    facts.extend(time.map(|time| ("Time", time)));
    facts.extend(number("gold").map(|gold| ("Gold", gold.to_string())));
    facts.extend(number("level").map(|level| ("Level", level.to_string())));
    facts.extend(health(number("health"), number("health_max")).map(|health| ("Health", health)));
    facts.extend(text("game_state").map(|state| ("State", state)));

    let opponent = situation.get("opponent").and_then(|opponent| serde_json::from_value::<SentOpponent>(opponent.clone()).ok()).map(|opponent| {
        let mut facts = Vec::new();
        facts.extend(opponent.hero.clone().filter(|hero| !hero.is_empty()).map(|hero| ("Hero", hero)));
        facts.extend(health(opponent.health, opponent.health_max).map(|health| ("Health", health)));
        OpponentView { facts, board: board_grid(&opponent.board, &opponent.slot_effects, names), skills: skill_chips(&opponent.skills, names) }
    });

    ContextView {
        summary: summary.join(" · "),
        sent: Some(Sent {
            facts,
            board,
            stash: cards("stash").map(|cards| stash_grid(&cards, names)),
            skills: cards("skills").map(|cards| skill_chips(&cards, names)),
            offer: cards("offer").map(|cards| cards.iter().map(|card| offer_card(card, names)).collect()),
            opponent,
            json: pretty(situation),
        }),
    }
}

/// `30/40`, or `30` without the most.
fn health(health: Option<i64>, most: Option<i64>) -> Option<String> {
    match (health, most) {
        (Some(health), Some(most)) => Some(format!("{health}/{most}")),
        (Some(health), None) => Some(health.to_string()),
        _ => None,
    }
}

/// The card as the game's board read would give it, for the board renderer. A card sent without an
/// id is known by its name only.
fn item(card: &SentCard, card_type: &str, on_board: bool) -> Item {
    Item {
        instance_id: card.name.clone(),
        template_id: card.id.clone(),
        card_type: card_type.to_owned(),
        tier: card.tier.clone().unwrap_or_default(),
        size: card.size.clone().unwrap_or_else(|| "Small".to_owned()),
        socket: if on_board { card.slot.map(|slot| format!("Socket_{slot}")) } else { None },
        enchantment: card.enchantment.clone(),
        ..Item::default()
    }
}

/// The name the situation gave the card with this id (the catalog may not know it).
fn sent_name<'a>(cards: &'a [SentCard], id: &str) -> Option<&'a str> {
    cards.iter().find(|card| card.id.as_deref() == Some(id)).map(|card| card.name.as_str()).filter(|name| !name.is_empty())
}

/// Each card on the slot it was sent with, and each slot effect marked on its slot.
fn board_grid(cards: &[SentCard], effects: &[SentSlotEffect], names: &Names) -> Grid {
    let mut items: Vec<Item> = cards.iter().map(|card| item(card, "Item", true)).collect();
    items.extend(effects.iter().map(|effect| Item {
        instance_id: effect.name.clone(),
        card_type: "SocketEffect".to_owned(),
        socket: Some(format!("Socket_{}", effect.slot)),
        ..Item::default()
    }));
    named(board::board_grid(&items, names), cards)
}

/// The cards in order: the stash has no slots.
fn stash_grid(cards: &[SentCard], names: &Names) -> Grid {
    let items: Vec<Item> = cards.iter().map(|card| item(card, "Item", false)).collect();
    named(board::stash_grid(&items, names), cards)
}

fn named(mut grid: Grid, cards: &[SentCard]) -> Grid {
    for tile in &mut grid.tiles {
        if let Some(name) = tile.template_id.as_deref().and_then(|id| sent_name(cards, id)) {
            tile.name = name.to_owned();
        }
    }
    grid
}

/// The skills as chips, rarest first, each with the name it was sent with.
fn skill_chips(cards: &[SentCard], names: &Names) -> Vec<SkillChip> {
    let items: Vec<Item> = cards.iter().map(|card| item(card, "Skill", false)).collect();
    let mut chips = board::skill_chips(&items, names);
    for chip in &mut chips {
        if let Some(name) = sent_name(cards, &chip.name) {
            chip.name = name.to_owned();
        }
    }
    chips
}

/// The card's name, with its enchantment first when it has one.
fn card_name(card: &SentCard, names: &Names) -> String {
    let name = if card.name.is_empty() { names.card(card.id.as_deref(), "?") } else { card.name.clone() };
    match &card.enchantment {
        Some(enchantment) => format!("{enchantment} {name}"),
        None => name,
    }
}

fn offer_card(card: &SentCard, names: &Names) -> OfferCard {
    OfferCard {
        name: card_name(card, names),
        tier: card.tier.as_deref().and_then(Tier::parse),
        kind: card.kind.as_deref().unwrap_or_default().replace('_', " "),
        art: names.art(card.id.as_deref()),
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
