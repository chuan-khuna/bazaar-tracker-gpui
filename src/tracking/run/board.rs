//! The board, stash and skills at one moment, as the tiles to draw: where each item sits on the
//! ten slots, its tier, its badges, its prices, and the lines of its tooltip.

use bazaar_tracker::memory::{Item, Snapshot};
use bazaar_tracker::run::Event;

use super::moment::{self, Choice};
use crate::tracking::names::Names;

/// How many slots the board has. The stash is drawn the same way (its real capacity is not known).
pub const SLOTS: u8 = 10;

/// A card's rarity, from the most common to the rarest: the order sorts by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Bronze,
    Silver,
    Gold,
    Diamond,
    Legendary,
}

impl Tier {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "Bronze" => Some(Tier::Bronze),
            "Silver" => Some(Tier::Silver),
            "Gold" => Some(Tier::Gold),
            "Diamond" => Some(Tier::Diamond),
            "Legendary" => Some(Tier::Legendary),
            _ => None,
        }
    }
}

/// What a badge counts. The UI gives each its own colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    Damage,
    Shield,
    Heal,
    Burn,
    Poison,
    Regen,
    Ammo,
    Multicast,
}

/// What a `SocketEffect` card does to its one slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    /// Jules's Stove: the item on it is Heated.
    Heat,
    /// Jules's Cooler: the item on it is Chilled.
    Chill,
    /// One of the dragon's Notes, A to G.
    Note,
    /// A socket effect not known here.
    Other,
}

/// A `SocketEffect` card on a board slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotMark {
    pub slot: u8,
    pub kind: SlotKind,
    /// `Heat`, `Chill`, a Note's letter, or the card's name.
    pub label: String,
    /// The card's name, for the tooltip: `Stove`, `B Note (Burn)`.
    pub name: String,
}

impl SlotMark {
    fn new(slot: u8, name: String) -> Self {
        let (kind, label) = match name.as_str() {
            "Stove" => (SlotKind::Heat, "Heat".to_owned()),
            "Cooler" => (SlotKind::Chill, "Chill".to_owned()),
            _ => match name.split_once(" Note") {
                Some((letter, _)) if letter.len() == 1 => (SlotKind::Note, letter.to_owned()),
                _ => (SlotKind::Other, name.clone()),
            },
        };
        Self { slot, kind, label, name }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    pub kind: BadgeKind,
    pub text: String,
}

/// One item on the grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    /// The first slot it covers, from 0.
    pub start: u8,
    /// How many slots it covers: 1, 2 or 3.
    pub span: u8,
    pub name: String,
    /// The card it is a copy of, for its odds.
    pub template_id: Option<String>,
    /// Its tier as the game names it (`Bronze`), for its odds.
    pub tier_name: String,
    /// Whether the card is an item (not an encounter or a skill): only items have odds.
    pub is_item: bool,
    pub enchantment: Option<String>,
    pub tier: Option<Tier>,
    pub badges: Vec<Badge>,
    pub sell_price: Option<i32>,
    /// `3s`, `4.5s`.
    pub cooldown: Option<String>,
    /// Everything else about the item.
    pub tooltip: Vec<String>,
    /// The card's extracted art: square, stretched to the card's shape when drawn.
    pub art: Option<std::path::PathBuf>,
}

/// A row of items laid out on the slots.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grid {
    /// In slot order.
    pub tiles: Vec<Tile>,
    /// Items that fitted nowhere.
    pub hidden: usize,
    /// The board's `SocketEffect` cards, each on its one slot, in slot order. The stash has none.
    pub marks: Vec<SlotMark>,
}

impl Grid {
    pub fn used_slots(&self) -> u8 {
        self.tiles.iter().map(|tile| tile.span).sum()
    }
}

/// A skill, drawn as a square icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillChip {
    pub name: String,
    pub tier: Option<Tier>,
    pub tooltip: Vec<String>,
    /// The skill's extracted icon.
    pub art: Option<std::path::PathBuf>,
}

/// The player's numbers at the moment, ready to print.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// `1240 / 1400`.
    pub health: Option<String>,
    /// `36 (+5)`: the gold, and the income it grows by each day when that was read.
    pub gold: Option<String>,
    pub level: Option<i32>,
}

/// Everything the board section draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardView {
    /// Which moment this is: `live`, `Event #4`, or in the Live section the game's state (`Shop`).
    pub moment: String,
    /// When in the run it is: `Day 7 · Hour 3`, `Day 7`, or `Hour 3` at the end of a chosen day.
    pub time: Option<String>,
    pub stats: Stats,
    /// The hero playing, as the game names it, and the day, when they were read.
    pub hero: Option<String>,
    pub day: Option<i32>,
    pub board: Grid,
    pub stash: Grid,
    /// `4 items · 5 of 10 slots`.
    pub stash_summary: String,
    pub skills: Vec<SkillChip>,
}

/// The board section for `events` at the moment `choice` names. `live` says whether the Run is the
/// live one.
pub fn view(events: &[Event], choice: Choice<'_>, live: bool, names: &Names) -> BoardView {
    let snapshot = moment::choose(events, choice);
    let empty = Snapshot::default();
    let shown = snapshot.unwrap_or(&empty);

    let moment = match (choice, snapshot) {
        (_, None) => "no board yet".to_owned(),
        (Choice::Event(id), _) => format!("Event #{}", events.iter().position(|event| event.id == id).map_or(0, |index| index + 1)),
        (Choice::Day(day), _) => format!("end of day {day}"),
        (Choice::Latest, _) if live => "live".to_owned(),
        (Choice::Latest, _) => "end of run".to_owned(),
    };
    let time = match (snapshot.is_some(), matches!(choice, Choice::Day(_))) {
        (false, _) => None,
        // The day is in the moment already.
        (true, true) => shown.stats.hour.map(|hour| format!("Hour {hour}")),
        (true, false) => time_of(shown.stats.day, shown.stats.hour),
    };
    view_of(shown, moment, time, names)
}

/// `Day 7 · Hour 3`, or `Day 7` without the hour; nothing without the day.
pub fn time_of(day: Option<i32>, hour: Option<i32>) -> Option<String> {
    match (day, hour) {
        (Some(day), Some(hour)) => Some(format!("Day {day} · Hour {hour}")),
        (Some(day), None) => Some(format!("Day {day}")),
        _ => None,
    }
}

/// The board section for this Snapshot, with `moment` and `time` saying which it is.
pub fn view_of(shown: &Snapshot, moment: String, time: Option<String>, names: &Names) -> BoardView {

    let stash = stash_grid(&shown.stash, names);
    let stash_summary = format!("{} · {} of {SLOTS} slots", plural(shown.stash.len(), "item"), stash.used_slots());
    BoardView {
        moment,
        time,
        stats: Stats {
            health: shown.stats.health.map(|health| match shown.stats.health_max {
                Some(max) => format!("{health} / {max}"),
                None => health.to_string(),
            }),
            gold: shown.stats.gold.map(|gold| match shown.stats.income {
                Some(income) if income >= 0 => format!("{gold} (+{income})"),
                Some(income) => format!("{gold} ({income})"),
                None => gold.to_string(),
            }),
            level: shown.stats.level,
        },
        hero: shown.stats.hero.clone(),
        day: shown.stats.day,
        board: board_grid(&shown.board, names),
        stash,
        stash_summary,
        skills: skill_chips(&shown.skills, names),
    }
}

/// A chip for each skill, in order.
/// The skills as chips, rarest first; skills of one tier keep the game's order, and a skill with
/// no known tier goes last.
pub fn skill_chips(skills: &[Item], names: &Names) -> Vec<SkillChip> {
    let mut chips: Vec<SkillChip> = skills.iter().map(|skill| SkillChip { name: names.card(skill.template_id.as_deref(), &skill.instance_id), tier: Tier::parse(&skill.tier), tooltip: tooltip(skill, &[]), art: names.art(skill.template_id.as_deref()) }).collect();
    // `None` sorts below every tier, so reversing puts it last.
    chips.sort_by_key(|chip| std::cmp::Reverse(chip.tier));
    chips
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 { format!("1 {noun}") } else { format!("{count} {noun}s") }
}

/// The board's items, each in its own socket. `SocketEffect` cards are not items: Jules's Stove
/// and Cooler and the dragon's Notes each mark one slot, so they become the grid's `marks`.
pub fn board_grid(items: &[Item], names: &Names) -> Grid {
    let is_effect = |item: &&Item| item.card_type == "SocketEffect";
    let mut grid = grid(items.iter().filter(|item| !is_effect(item)), names, true);
    grid.marks = items.iter().filter(is_effect).filter_map(|item| Some(SlotMark::new(socket_of(item).filter(|slot| *slot < SLOTS)?, names.card(item.template_id.as_deref(), &item.instance_id)))).collect();
    grid.marks.sort_by_key(|mark| mark.slot);
    grid
}

/// The stash's items, in order: the stash has no sockets.
pub fn stash_grid(items: &[Item], names: &Names) -> Grid {
    grid(items.iter(), names, false)
}

fn grid<'a>(items: impl Iterator<Item = &'a Item>, names: &Names, by_socket: bool) -> Grid {
    let items: Vec<&Item> = items.collect();
    let wanted: Vec<(Option<u8>, u8)> = items.iter().map(|item| (if by_socket { socket_of(item) } else { None }, span_of(item))).collect();
    let starts = place(&wanted);
    let mut tiles: Vec<Tile> = items.iter().zip(&wanted).zip(starts).filter_map(|((item, (_, span)), start)| Some(tile(item, start?, *span, names))).collect();
    let hidden = items.len() - tiles.len();
    tiles.sort_by_key(|tile| tile.start);
    Grid { tiles, hidden, marks: Vec::new() }
}

/// The slot an item names for itself: `Socket_6` is slot 6.
fn socket_of(item: &Item) -> Option<u8> {
    item.socket.as_deref()?.strip_prefix("Socket_")?.parse().ok()
}

fn span_of(item: &Item) -> u8 {
    match item.size.as_str() {
        "Medium" => 2,
        "Large" => 3,
        _ => 1,
    }
}

/// Where each item starts, given the slot it wants (if any) and how many it covers. Items whose
/// wanted slots are free get them; the rest take the first gap they fit, in order. `None` is an
/// item that fits nowhere.
fn place(wanted: &[(Option<u8>, u8)]) -> Vec<Option<u8>> {
    let mut taken = [false; SLOTS as usize];
    let mut starts = vec![None; wanted.len()];
    let fits = |taken: &[bool; SLOTS as usize], start: u8, span: u8| start + span <= SLOTS && taken[start as usize..(start + span) as usize].iter().all(|taken| !taken);
    let take = |taken: &mut [bool; SLOTS as usize], start: u8, span: u8| taken[start as usize..(start + span) as usize].fill(true);
    for (index, (socket, span)) in wanted.iter().enumerate() {
        if let Some(start) = *socket
            && fits(&taken, start, *span)
        {
            take(&mut taken, start, *span);
            starts[index] = Some(start);
        }
    }
    for (index, (_, span)) in wanted.iter().enumerate() {
        if starts[index].is_none()
            && let Some(start) = (0..SLOTS).find(|start| fits(&taken, *start, *span))
        {
            take(&mut taken, start, *span);
            starts[index] = Some(start);
        }
    }
    starts
}

fn tile(item: &Item, start: u8, span: u8, names: &Names) -> Tile {
    let attribute = |name: &str| item.attributes.get(name).copied().unwrap_or(0);
    let mut badges = Vec::new();
    let mut shown: Vec<&str> = Vec::new();
    let mut badge = |kind: BadgeKind, attribute_name: &'static str, text: String| {
        badges.push(Badge { kind, text });
        shown.push(attribute_name);
    };
    for (kind, name) in [
        (BadgeKind::Damage, "DamageAmount"),
        (BadgeKind::Shield, "ShieldApplyAmount"),
        (BadgeKind::Heal, "HealAmount"),
        (BadgeKind::Burn, "BurnApplyAmount"),
        (BadgeKind::Poison, "PoisonApplyAmount"),
        (BadgeKind::Regen, "RegenApplyAmount"),
    ] {
        if attribute(name) > 0 {
            badge(kind, name, attribute(name).to_string());
        }
    }
    if attribute("AmmoMax") > 0 {
        badge(BadgeKind::Ammo, "Ammo", attribute("Ammo").to_string());
    }
    if attribute("Multicast") > 1 {
        badge(BadgeKind::Multicast, "Multicast", format!("x{}", attribute("Multicast")));
    }
    let sell_price = Some(attribute("SellPrice")).filter(|price| *price > 0);
    if sell_price.is_some() {
        shown.push("SellPrice");
    }
    let cooldown = Some(attribute("CooldownMax")).filter(|ms| *ms > 0).map(seconds);
    if cooldown.is_some() {
        shown.push("CooldownMax");
    }
    Tile {
        start,
        span,
        name: names.card(item.template_id.as_deref(), &item.instance_id),
        template_id: item.template_id.clone(),
        tier_name: item.tier.clone(),
        is_item: item.card_type == "Item",
        enchantment: item.enchantment.clone(),
        tier: Tier::parse(&item.tier),
        badges,
        sell_price,
        cooldown,
        tooltip: tooltip(item, &shown),
        art: names.art(item.template_id.as_deref()),
    }
}

/// Milliseconds as `3s` or `4.5s`.
fn seconds(ms: i32) -> String {
    if ms % 1000 == 0 { format!("{}s", ms / 1000) } else { format!("{:.1}s", f64::from(ms) / 1000.) }
}

/// The lines of an item's tooltip: what it is, then every non-zero attribute the tile does not
/// already show (`shown`).
pub fn tooltip(item: &Item, shown: &[&str]) -> Vec<String> {
    let mut lines = vec![format!("{} · {} · {}", item.tier, item.size, item.card_type)];
    if let Some(enchantment) = &item.enchantment {
        lines.push(format!("Enchantment: {enchantment}"));
    }
    if !item.tags.is_empty() {
        lines.push(format!("Tags: {}", item.tags.join(", ")));
    }
    lines.extend(item.attributes.iter().filter(|(name, value)| **value != 0 && !shown.contains(&name.as_str())).map(|(name, value)| format!("{name}: {value}")));
    lines
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
