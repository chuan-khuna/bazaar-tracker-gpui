//! The board, stash and skills at one moment, as the tiles to draw: where each item sits on the
//! ten slots, its tier, its badges, its prices, and the lines of its tooltip.

use bazaar_tracker::memory::{Item, Snapshot};
use bazaar_tracker::run::Event;

use super::moment::{self, Choice};
use super::names::Names;

/// How many slots the board has. The stash is drawn the same way (its real capacity is not known).
pub const SLOTS: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub enchantment: Option<String>,
    pub tier: Option<Tier>,
    pub badges: Vec<Badge>,
    pub sell_price: Option<i32>,
    /// `3s`, `4.5s`.
    pub cooldown: Option<String>,
    /// Everything else about the item.
    pub tooltip: Vec<String>,
}

/// A row of items laid out on the slots.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grid {
    /// In slot order.
    pub tiles: Vec<Tile>,
    /// Items that fitted nowhere.
    pub hidden: usize,
}

impl Grid {
    pub fn used_slots(&self) -> u8 {
        self.tiles.iter().map(|tile| tile.span).sum()
    }
}

/// A skill, drawn as a chip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillChip {
    pub name: String,
    pub tier: Option<Tier>,
    pub tooltip: Vec<String>,
}

/// The player's numbers at the moment, ready to print.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// `1240 / 1400`.
    pub health: Option<String>,
    pub gold: Option<i32>,
    pub level: Option<i32>,
}

/// Everything the board section draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardView {
    /// Which moment this is: `live · Day 7 Hour 3`.
    pub moment: String,
    pub stats: Stats,
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

    let mut moment = match (choice, snapshot) {
        (_, None) => "no board yet".to_owned(),
        (Choice::Event(id), _) => format!("Event #{}", events.iter().position(|event| event.id == id).map_or(0, |index| index + 1)),
        (Choice::Day(day), _) => format!("end of day {day}"),
        (Choice::Latest, _) if live => "live".to_owned(),
        (Choice::Latest, _) => "end of run".to_owned(),
    };
    if snapshot.is_some() {
        match (shown.stats.day, shown.stats.hour, matches!(choice, Choice::Day(_))) {
            (Some(day), Some(hour), false) => moment.push_str(&format!(" · Day {day} Hour {hour}")),
            (Some(day), None, false) => moment.push_str(&format!(" · Day {day}")),
            (_, Some(hour), true) => moment.push_str(&format!(" · Hour {hour}")),
            _ => {}
        }
    }
    view_of(shown, moment, names)
}

/// The board section for this Snapshot, with `moment` as the words saying which it is.
pub fn view_of(shown: &Snapshot, moment: String, names: &Names) -> BoardView {

    let stash = stash_grid(&shown.stash, names);
    let stash_summary = format!("{} · {} of {SLOTS} slots", plural(shown.stash.len(), "item"), stash.used_slots());
    BoardView {
        moment,
        stats: Stats {
            health: shown.stats.health.map(|health| match shown.stats.health_max {
                Some(max) => format!("{health} / {max}"),
                None => health.to_string(),
            }),
            gold: shown.stats.gold,
            level: shown.stats.level,
        },
        board: board_grid(&shown.board, names),
        stash,
        stash_summary,
        skills: shown.skills.iter().map(|skill| SkillChip { name: names.card(skill.template_id.as_deref(), &skill.instance_id), tier: Tier::parse(&skill.tier), tooltip: tooltip(skill, &[]) }).collect(),
    }
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 { format!("1 {noun}") } else { format!("{count} {noun}s") }
}

/// The board's items, each in its own socket. `SocketEffect` cards are left out: the game's board
/// holds them but does not show them (record 2026-10-31.002).
pub fn board_grid(items: &[Item], names: &Names) -> Grid {
    grid(items.iter().filter(|item| item.card_type != "SocketEffect"), names, true)
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
    Grid { tiles, hidden }
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
        enchantment: item.enchantment.clone(),
        tier: Tier::parse(&item.tier),
        badges,
        sell_price,
        cooldown,
        tooltip: tooltip(item, &shown),
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
