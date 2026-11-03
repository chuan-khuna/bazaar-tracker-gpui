//! The board, stash and skills of the selected Run at the moment being looked at: items on a
//! ten-slot grid with a tier border, their art, badges, prices and a tooltip. A slot is half as wide
//! as it is tall, so a small item is w = 0.5h, a medium one w = h and a large one w = 1.5h, the
//! shapes the square art is stretched to. Skills are square icons.

use gpui::{Context, Div, ObjectFit, SharedString, Stateful, div, img, prelude::*, px, relative, rgb, rgba};
use gpui_base::{ScrollbarAxis, ScrollbarMode};

use crate::ui::TrackerApp;
use crate::ui::item_odds::{OddsContext, TILE_GROUP};
use crate::ui::widgets::{self, Tone};
use crate::shell::settings::CardSize;
use crate::theme::Theme;
use crate::tracking::run::moment::Choice;
use crate::tracking::run::board::{BadgeKind, BoardView, Grid, SLOTS, SkillChip, SlotKind, Tier, Tile};

/// Which side of a grid its slot marks go: the game puts the player's under their items and the
/// opponent's over theirs.
#[derive(Clone, Copy)]
pub(in crate::ui) enum MarkSide {
    Above,
    Below,
}

/// The height of the row of slot marks (Stove, Cooler, Notes).
const SLOT_MARK_HEIGHT: f32 = 13.;

/// A slot's width over its height.
const SLOT_ASPECT: f32 = 0.5;

/// How many skills share a row, and how many sit together in a group within it.
const SKILLS_PER_ROW: usize = 8;
const SKILLS_PER_GROUP: usize = 4;

/// The most width a row of items or skills takes at each card size: ten slots of 48, 72 or 96 px
/// (half, three quarters and all of Large). A wider panel leaves the rest empty, so the tiles do
/// not stretch across a wide window.
fn max_row_width(size: CardSize) -> f32 {
    match size {
        CardSize::Small => 480.,
        CardSize::Medium => 720.,
        CardSize::Large => 960.,
    }
}

/// The least width of a row of items or skills: two thirds of the most. A narrower row scrolls
/// sideways.
fn min_row_width(size: CardSize) -> f32 {
    max_row_width(size) * 2. / 3.
}

/// The space under a sideways row for its scroll bar (a 6 px thumb, 3 px from the edge), so the
/// bar runs under the row and not over its tiles.
const SCROLLBAR_ROOM: f32 = 10.;

fn tier_colour(t: &Theme, tier: Option<Tier>) -> u32 {
    match tier {
        Some(Tier::Bronze) => t.bronze,
        Some(Tier::Silver) => t.silver,
        Some(Tier::Gold) => t.gold,
        Some(Tier::Diamond) => t.diamond,
        Some(Tier::Legendary) => t.legendary,
        None => t.highlight_med,
    }
}

/// The colour of the game's state in the Live header: red for a fight, gold for a shop.
fn phase_colour(t: &Theme, phase: &str) -> u32 {
    match phase {
        "Combat" => t.loss,
        "Shop" => t.gold,
        "Encounter choice" => t.accent,
        "Loot" => t.heal,
        _ => t.subtle,
    }
}

fn badge_colour(t: &Theme, kind: BadgeKind) -> u32 {
    match kind {
        BadgeKind::Damage => t.damage,
        BadgeKind::Shield => t.shield,
        BadgeKind::Heal => t.heal,
        BadgeKind::Burn => t.burn,
        BadgeKind::Poison => t.poison,
        BadgeKind::Regen => t.regen,
        BadgeKind::Ammo => t.ammo,
        BadgeKind::Multicast => t.overlay,
    }
}

/// Which board section is drawn.
#[derive(Clone, Copy)]
pub(in crate::ui) enum Section {
    /// The game as it is now. Without `with_stash` the stash and its heading are left out (as in a
    /// combat).
    Live { with_stash: bool },
    /// The selected Run at the chosen moment. A chevron on the right of its header folds it away.
    Snapshot,
}

impl TrackerApp {
    /// A section: its title and the moment it shows, then `extra`, then the board, stash and skills.
    pub(in crate::ui) fn render_board_section(&self, view: &BoardView, section: Section, extra: Vec<Div>, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let (prefix, with_stash, fold) = match section {
            Section::Live { with_stash } => ("live", with_stash, None),
            Section::Snapshot => ("snapshot", true, Some(self.settings.get().window.snapshot_open)),
        };
        // The title (LIVE as its badge), then which moment it is and when: the game's state is in
        // its own colour.
        let (title, moment_colour) = match section {
            Section::Live { .. } => (widgets::live_badge(t), phase_colour(t, &view.moment)),
            Section::Snapshot => (widgets::section_title(t, "SNAPSHOT"), t.subtle),
        };
        let mut left = vec![title, widgets::badge(view.moment.clone(), moment_colour, Tone::Soft, t)];
        left.extend(view.time.clone().map(|time| widgets::badge(time, t.subtle, Tone::Soft, t)));
        // Looking back at an Event or a day: a button back to the board as it is now.
        let looking_back = matches!(section, Section::Snapshot) && self.session.choice() != Choice::Latest;
        if looking_back {
            let live = self.session.selected_run().is_some_and(|run| self.session.is_live(&run.id));
            let label = if live { "Go live" } else { "Go to latest" };
            left.push(div().child(self.filter_button("snapshot-latest".to_owned(), label.to_owned(), false).on_click(cx.listener(|this, _, _, cx| {
                // Not a click on the header, which would fold the section.
                cx.stop_propagation();
                this.show_latest(cx);
            }))));
        }
        // The player's stats on the right, each in its own colour.
        let mut right: Vec<Div> = Vec::new();
        right.extend(view.stats.health.clone().map(|health| widgets::badge(format!("♥ {health}"), t.loss, Tone::Soft, t)));
        right.extend(view.stats.gold.clone().map(|gold| widgets::badge(format!("● {gold} gold"), t.gold, Tone::Soft, t)));
        right.extend(view.stats.level.map(|level| widgets::badge(format!("Lv {level}"), t.subtle, Tone::Soft, t)));
        right.extend(fold.map(|open| div().text_xs().text_color(rgb(t.muted)).child(if open { "▾" } else { "▸" })));
        // Whose odds the items' buttons ask for: the hero read with the board, else the Run's.
        let run_hero = self.session.selected_run().and_then(|run| run.hero.clone());
        let odds = OddsContext::new(view.hero.as_deref().or(run_hero.as_deref()), view.day);
        let open = fold.unwrap_or(true);
        let stash_open = with_stash && self.settings.get().window.stash_open;
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_1p5()
            .px_3()
            .py_2()
            .child(
                div()
                    .id(SharedString::from(format!("{prefix}-heading")))
                    .pb_1()
                    .when(fold.is_some(), |heading| heading.cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.toggle_snapshot(cx))))
                    .child(widgets::section_header(left, right)),
            )
            .when(open, |section| section.children(extra).child(self.render_grid(format!("{prefix}-board"), &view.board, MarkSide::Below, odds.as_ref(), cx)))
            .when(open && with_stash, |section| section.child(
                div()
                    .id(SharedString::from(format!("{prefix}-stash-heading")))
                    .flex()
                    .items_center()
                    .gap_3()
                    .mt_1()
                    .text_xs()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_stash(cx)))
                    .child(widgets::section_title(t, if stash_open { "▾ STASH" } else { "▸ STASH" }))
                    .child(div().text_color(rgb(t.subtle)).child(view.stash_summary.clone())),
            ))
            .when(open && stash_open, |section| section.child(self.render_grid(format!("{prefix}-stash"), &view.stash, MarkSide::Below, odds.as_ref(), cx)))
            .when(open, |section| section.child(self.render_skills(prefix, &view.skills)))
    }

    /// The most width a row of cards takes at the chosen card size. Item odds badges take their
    /// width from it, so they keep one size while the panel around them grows.
    pub(in crate::ui) fn max_row_width(&self) -> f32 {
        max_row_width(self.settings.get().card_size)
    }

    /// `content` in a strip that scrolls sideways when it is narrower than [`min_row_width`] and is
    /// no wider than [`max_row_width`] for the chosen card size, with its own scroll bar. The bar
    /// stays in view while the row overflows: the mouse wheel is left to the panel around it, so the
    /// row moves by dragging the bar, by Shift and the wheel, or by a sideways swipe.
    pub(in crate::ui) fn sideways(&self, id: String, content: Div) -> Div {
        let handle = self.sideways_scrolls.borrow_mut().entry(id.clone()).or_default().clone();
        let size = self.settings.get().card_size;
        div()
            .relative()
            .w_full()
            .child(div().id(SharedString::from(format!("{id}-strip"))).w_full().pb(px(SCROLLBAR_ROOM)).overflow_x_scroll().restrict_scroll_to_axis().track_scroll(&handle).child(content.w_full().min_w(px(min_row_width(size))).max_w(px(max_row_width(size)))))
            .child(widgets::scrollbar(self.theme, SharedString::from(format!("{id}-scrollbar")), &handle).axis(ScrollbarAxis::Horizontal).mode(ScrollbarMode::Always))
    }

    /// Ten empty slots, with the grid's items on them, in a strip of their own that scrolls sideways.
    /// The slots a Stove, Cooler or Note sits on are marked on the `side` of the grid: under the
    /// player's slots, over the opponent's.
    pub(in crate::ui) fn render_grid(&self, id: String, grid: &Grid, side: MarkSide, odds: Option<&OddsContext>, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let slots = f32::from(SLOTS);
        let strip = id.clone();
        let marks = (!grid.marks.is_empty()).then(|| {
            div().relative().h(px(SLOT_MARK_HEIGHT)).children(grid.marks.iter().map(|mark| {
                let (background, text) = match mark.kind {
                    SlotKind::Heat => (t.burn, t.background),
                    SlotKind::Chill => (t.diamond, t.background),
                    SlotKind::Note => (t.legendary, t.background),
                    SlotKind::Other => (t.overlay, t.text),
                };
                div()
                    .id(SharedString::from(format!("{id}-mark-{}", mark.slot)))
                    .absolute()
                    .top_0()
                    .h_full()
                    .left(relative(f32::from(mark.slot) / slots))
                    .w(relative(1. / slots))
                    .px(px(2.))
                    .tooltip(widgets::hint(t, vec![mark.name.clone()]))
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .overflow_hidden()
                            .rounded_sm()
                            .bg(rgb(background))
                            .text_color(rgb(text))
                            .text_size(px(9.))
                            .font_weight(gpui::FontWeight::BOLD)
                            .whitespace_nowrap()
                            .child(mark.label.clone()),
                    )
            }))
        });
        let (above, below) = match side {
            MarkSide::Above => (marks, None),
            MarkSide::Below => (None, marks),
        };
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(self.sideways(
                strip,
                div().flex().flex_col().gap(px(1.)).children(above).child(
                    div()
                        .relative()
                        .w_full()
                        .aspect_ratio(f32::from(SLOTS) * SLOT_ASPECT)
                        .child(div().absolute().inset_0().flex().children((0..SLOTS).map(|_| div().flex_1().m(px(2.)).rounded_md().border_1().border_color(rgb(t.highlight_low)))))
                        .children(grid.tiles.iter().enumerate().map(|(index, tile)| {
                            div()
                                .id(SharedString::from(format!("{id}-{index}")))
                                .absolute()
                                .top_0()
                                .h_full()
                                .left(relative(f32::from(tile.start) / slots))
                                .w(relative(f32::from(tile.span) / slots))
                                .child(self.render_tile(SharedString::from(format!("{id}-{index}-odds")), tile, odds, cx))
                        })),
                ).children(below),
            ))
            .when(grid.hidden > 0, |column| column.child(div().text_xs().text_color(rgb(t.muted)).child(format!("{} not shown", grid.hidden))))
    }

    fn render_tile(&self, odds_id: SharedString, tile: &Tile, odds: Option<&OddsContext>, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let mut lines = vec![match &tile.enchantment {
            Some(enchantment) => format!("{enchantment} {}", tile.name),
            None => tile.name.clone(),
        }];
        lines.extend(tile.tooltip.iter().cloned());
        let has_art = tile.art.is_some();
        let has_footer = tile.sell_price.is_some() || tile.cooldown.is_some() || (has_art && tile.enchantment.is_some());
        let odds_button = odds.and_then(|odds| self.odds_button(odds_id, tile, odds, cx));
        div()
            .id("tile")
            .group(TILE_GROUP)
            .relative()
            .m(px(2.))
            .size_full()
            .rounded_md()
            .border_2()
            .border_color(rgb(tier_colour(t, tile.tier)))
            .bg(rgb(t.highlight_low))
            .overflow_hidden()
            .tooltip(widgets::hint(t, lines))
            // The art is square: like the game, stretch it to the card's shape.
            .children(tile.art.clone().map(|art| img(art).absolute().inset_0().size_full().object_fit(ObjectFit::Fill)))
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left_0()
                    .right_0()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(2.))
                    .children(tile.badges.iter().map(|badge| {
                        let background = badge_colour(t, badge.kind);
                        let text = if badge.kind == BadgeKind::Multicast { t.text } else { t.background };
                        div().px_1().rounded_sm().bg(rgb(background)).text_color(rgb(text)).text_size(px(10.)).font_weight(gpui::FontWeight::BOLD).child(badge.text.clone())
                    })),
            )
            // Without art, the name is the card; with it, the name is in the tooltip.
            .when(!has_art, |card| {
                card.child(
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .px_1()
                        .pt_3()
                        .pb_2()
                        .text_center()
                        .text_size(px(11.))
                        .line_height(px(13.))
                        .text_color(rgb(t.text))
                        .children(tile.enchantment.clone().map(|enchantment| div().text_color(rgb(t.accent)).child(enchantment)))
                        .child(tile.name.clone()),
                )
            })
            // A dark band along the bottom keeps the price and cooldown readable over the art.
            .when(has_footer, |card| {
                card.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(3.))
                        .h(px(15.))
                        .when(has_art, |band| band.bg(rgba(0x0000_00b0)))
                        .text_size(px(10.))
                        .child(div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.value)).children(tile.sell_price.map(|price| price.to_string())))
                        .children(tile.enchantment.clone().filter(|_| has_art).map(|enchantment| div().min_w_0().overflow_hidden().whitespace_nowrap().text_color(rgb(t.accent)).child(enchantment)))
                        .child(div().text_color(rgb(t.subtle)).children(tile.cooldown.clone())),
                )
            })
            // Over everything else, so it takes the click.
            .children(odds_button)
    }

    /// The skills as square icons beside a SKILLS label: eight to a row across half its width, in two groups of four. The
    /// tooltip names the skill. A skill with no icon shows the start of its name.
    pub(in crate::ui) fn render_skills(&self, prefix: &'static str, skills: &[SkillChip]) -> Div {
        let t = self.theme;
        let icon = |index: usize, skill: &SkillChip| {
            let mut lines = vec![skill.name.clone()];
            lines.extend(skill.tooltip.iter().cloned());
            let tier = tier_colour(t, skill.tier);
            let tint = move |alpha: u32| rgba((tier << 8) | alpha);
            div()
                .id(SharedString::from(format!("{prefix}-skill-{index}")))
                .relative()
                .size_full()
                .rounded_md()
                .border_2()
                .border_color(rgb(tier))
                .bg(tint(0x1a))
                .overflow_hidden()
                .hover(move |style| style.bg(tint(0x33)))
                .tooltip(widgets::hint(t, lines))
                .map(|square| match skill.art.clone() {
                    Some(art) => square.child(img(art).size_full().object_fit(ObjectFit::Cover)),
                    None => square.flex().items_center().justify_center().p_0p5().text_center().text_size(px(10.)).line_height(px(12.)).text_color(rgb(t.text)).child(skill.name.clone()),
                })
        };
        // A row's cells, with a wider gap between its two groups; blank cells keep a short row in step.
        let row = |row: usize, line: &[SkillChip]| {
            // Half the width beside the label, so an icon is about a third of an item card's height.
            div().w_1_2().flex().gap_2().children((0..SKILLS_PER_ROW / SKILLS_PER_GROUP).map(move |group| {
                div().flex().flex_1().gap_1().children((0..SKILLS_PER_GROUP).map(move |slot| {
                    let column = group * SKILLS_PER_GROUP + slot;
                    div().flex_1().min_w_0().aspect_square().children(line.get(column).map(|skill| icon(row * SKILLS_PER_ROW + column, skill)))
                }))
            }))
        };
        self.sideways(
            format!("{prefix}-skills"),
            div()
                .flex()
                .items_start()
                .gap_1p5()
                .text_xs()
                .child(div().flex_none().w(px(44.)).pt_1().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child("SKILLS"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .children(skills.chunks(SKILLS_PER_ROW).enumerate().map(|(index, line)| row(index, line)))
                        .when(skills.is_empty(), |rows| rows.child(div().py_0p5().text_color(rgb(t.muted)).child("none"))),
                ),
        )
    }
}
