//! The board, stash and skills of the selected Run at the moment being looked at: items on a
//! ten-slot grid with a tier border, badges, prices and a tooltip. There is no card art.

use gpui::{Context, Div, SharedString, Stateful, div, prelude::*, px, relative, rgb};
use gpui_base::{ScrollbarAxis, ScrollbarMode};

use crate::ui::TrackerApp;
use crate::ui::widgets;
use crate::theme::Theme;
use crate::tracking::run::board::{BadgeKind, BoardView, Grid, SLOTS, SkillChip, Tier, Tile};

/// How tall a row of the grid is.
const TILE_HEIGHT: f32 = 78.;

/// How many skills share a row.
const SKILLS_PER_ROW: usize = 5;

/// The least width of a row of items or skills: ten slots of 64 px. A narrower row scrolls sideways.
const MIN_ROW_WIDTH: f32 = 640.;

/// The most width a row of items or skills takes: ten slots of 96 px. A wider panel leaves the rest
/// empty, so the tiles do not stretch across a wide window.
pub(in crate::ui) const MAX_ROW_WIDTH: f32 = 960.;

/// The space under a sideways row for its scroll bar (a 6 px thumb, 3 px from the edge), so the
/// bar runs under the row and not over its tiles.
const SCROLLBAR_ROOM: f32 = 10.;

/// How tall a skill chip is, so that every row of them lines up.
const SKILL_HEIGHT: f32 = 22.;

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
        let (prefix, title, with_stash, fold) = match section {
            Section::Live { with_stash } => ("live", "LIVE", with_stash, None),
            Section::Snapshot => ("snapshot", "SNAPSHOT", true, Some(self.settings.get().window.snapshot_open)),
        };
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
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .when(fold.is_some(), |heading| heading.cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.toggle_snapshot(cx))))
                    .child(div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(title))
                    .child(div().text_color(rgb(t.subtle)).child(view.moment.clone()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .ml_auto()
                            .children(view.stats.health.clone().map(|health| div().text_color(rgb(t.loss)).child(format!("♥ {health}"))))
                            .children(view.stats.gold.clone().map(|gold| div().text_color(rgb(t.gold)).child(format!("● {gold} gold"))))
                            .children(view.stats.level.map(|level| div().text_color(rgb(t.subtle)).child(format!("Lv {level}"))))
                            .children(fold.map(|open| div().text_color(rgb(t.muted)).child(if open { "▾" } else { "▸" }))),
                    ),
            )
            .when(open, |section| section.children(extra).child(self.render_grid(format!("{prefix}-board"), &view.board)))
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
                    .child(div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(if stash_open { "▾ STASH" } else { "▸ STASH" }))
                    .child(div().text_color(rgb(t.subtle)).child(view.stash_summary.clone())),
            ))
            .when(open && stash_open, |section| section.child(self.render_grid(format!("{prefix}-stash"), &view.stash)))
            .when(open, |section| section.child(self.render_skills(prefix, &view.skills)))
    }

    /// `content` in a strip that scrolls sideways when it is narrower than [`MIN_ROW_WIDTH`] and is
    /// no wider than [`MAX_ROW_WIDTH`], with its own scroll bar. The bar stays in view while the row
    /// overflows: the mouse wheel is left to the panel around it, so the row moves by dragging the
    /// bar, by Shift and the wheel, or by a sideways swipe.
    pub(in crate::ui) fn sideways(&self, id: String, content: Div) -> Div {
        let handle = self.sideways_scrolls.borrow_mut().entry(id.clone()).or_default().clone();
        div()
            .relative()
            .w_full()
            .child(div().id(SharedString::from(format!("{id}-strip"))).w_full().pb(px(SCROLLBAR_ROOM)).overflow_x_scroll().restrict_scroll_to_axis().track_scroll(&handle).child(content.w_full().min_w(px(MIN_ROW_WIDTH)).max_w(px(MAX_ROW_WIDTH))))
            .child(widgets::scrollbar(self.theme, SharedString::from(format!("{id}-scrollbar")), &handle).axis(ScrollbarAxis::Horizontal).mode(ScrollbarMode::Always))
    }

    /// Ten empty slots, with the grid's items on them, in a strip of their own that scrolls sideways.
    pub(in crate::ui) fn render_grid(&self, id: String, grid: &Grid) -> Div {
        let t = self.theme;
        let slots = f32::from(SLOTS);
        let strip = id.clone();
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(self.sideways(
                strip,
                div()
                    .relative()
                    .h(px(TILE_HEIGHT))
                    .child(div().absolute().inset_0().flex().children((0..SLOTS).map(|_| div().flex_1().m(px(2.)).rounded_md().border_1().border_color(rgb(t.highlight_low)))))
                    .children(grid.tiles.iter().enumerate().map(|(index, tile)| {
                        div()
                            .id(SharedString::from(format!("{id}-{index}")))
                            .absolute()
                            .top_0()
                            .h_full()
                            .left(relative(f32::from(tile.start) / slots))
                            .w(relative(f32::from(tile.span) / slots))
                            .child(self.render_tile(tile))
                    })),
            ))
            .when(grid.hidden > 0, |column| column.child(div().text_xs().text_color(rgb(t.muted)).child(format!("{} not shown", grid.hidden))))
    }

    fn render_tile(&self, tile: &Tile) -> Stateful<Div> {
        let t = self.theme;
        let mut lines = vec![match &tile.enchantment {
            Some(enchantment) => format!("{enchantment} {}", tile.name),
            None => tile.name.clone(),
        }];
        lines.extend(tile.tooltip.iter().cloned());
        div()
            .id("tile")
            .relative()
            .m(px(2.))
            .size_full()
            .rounded_md()
            .border_2()
            .border_color(rgb(tier_colour(t, tile.tier)))
            .bg(rgb(t.highlight_low))
            .overflow_hidden()
            .tooltip(widgets::hint(t, lines))
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .gap(px(2.))
                    .children(tile.badges.iter().map(|badge| {
                        let background = badge_colour(t, badge.kind);
                        let text = if badge.kind == BadgeKind::Multicast { t.text } else { t.background };
                        div().px_1().rounded_sm().bg(rgb(background)).text_color(rgb(text)).text_size(px(10.)).font_weight(gpui::FontWeight::BOLD).child(badge.text.clone())
                    })),
            )
            .child(
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
            .children(tile.sell_price.map(|price| div().absolute().bottom(px(1.)).left(px(3.)).text_size(px(10.)).font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.value)).child(price.to_string())))
            .children(tile.cooldown.clone().map(|cooldown| div().absolute().bottom(px(1.)).right(px(3.)).text_size(px(10.)).text_color(rgb(t.subtle)).child(cooldown)))
    }

    /// The skills, five to a row in equal columns, beside a SKILLS label. A long name is cut short;
    /// the tooltip has it whole.
    pub(in crate::ui) fn render_skills(&self, prefix: &'static str, skills: &[SkillChip]) -> Div {
        let t = self.theme;
        let chip = |index: usize, skill: &SkillChip| {
            let mut lines = vec![skill.name.clone()];
            lines.extend(skill.tooltip.iter().cloned());
            // Tinted with the tier's colour: a soft fill, a faint edge and a solid stripe on the
            // left; the fill deepens under the pointer.
            let tier = tier_colour(t, skill.tier);
            let tint = move |alpha: u32| gpui::rgba((tier << 8) | alpha);
            div()
                .id(SharedString::from(format!("{prefix}-skill-{index}")))
                .flex()
                .w_full()
                .min_w_0()
                .items_center()
                .h(px(SKILL_HEIGHT))
                .pl_1p5()
                .pr_2()
                .rounded_md()
                .border_1()
                .border_l_4()
                .border_color(tint(0x66))
                .bg(tint(0x1a))
                .hover(move |style| style.bg(tint(0x33)))
                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(rgb(t.text)).child(skill.name.clone()))
                .tooltip(widgets::hint(t, lines))
        };
        self.sideways(
            format!("{prefix}-skills"),
            div()
                .flex()
                .items_start()
                .gap_1p5()
                .text_xs()
                .child(div().flex().flex_none().items_center().w(px(44.)).h(px(SKILL_HEIGHT)).font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child("SKILLS"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .children(skills.chunks(SKILLS_PER_ROW).enumerate().map(|(row, line)| {
                            div()
                                .flex()
                                .gap_1()
                                .children(line.iter().enumerate().map(|(column, skill)| div().flex_1().min_w_0().child(chip(row * SKILLS_PER_ROW + column, skill))))
                                // Blank cells keep a short last row in the same columns.
                                .children((line.len()..SKILLS_PER_ROW).map(|_| div().flex_1().min_w_0()))
                        }))
                        .when(skills.is_empty(), |rows| rows.child(div().py_0p5().text_color(rgb(t.muted)).child("none"))),
                ),
        )
    }
}
