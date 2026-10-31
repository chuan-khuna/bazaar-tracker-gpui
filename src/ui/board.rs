//! The board, stash and skills of the selected Run at the moment being looked at: items on a
//! ten-slot grid with a tier border, badges, prices and a tooltip. There is no card art.

use gpui::{Context, Div, SharedString, Stateful, div, prelude::*, px, relative, rgb};

use super::TrackerApp;
use super::widgets;
use crate::theme::Theme;
use crate::tracking::board::{BadgeKind, BoardView, Grid, SLOTS, SkillChip, Tier, Tile};

/// How tall a row of the grid is.
const TILE_HEIGHT: f32 = 78.;

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

impl TrackerApp {
    /// A section: its title and the moment it shows, then `extra`, then the board, stash and skills.
    /// `prefix` keeps the element ids of two sections apart.
    pub(super) fn render_board_section(&self, view: &BoardView, prefix: &'static str, title: &'static str, extra: Vec<Div>, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let stash_open = self.settings.get().window.stash_open;
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_1p5()
            .px_3()
            .py_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(title))
                    .child(div().text_color(rgb(t.subtle)).child(view.moment.clone()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .ml_auto()
                            .children(view.stats.health.clone().map(|health| div().text_color(rgb(t.loss)).child(format!("♥ {health}"))))
                            .children(view.stats.gold.map(|gold| div().text_color(rgb(t.gold)).child(format!("● {gold} gold"))))
                            .children(view.stats.level.map(|level| div().text_color(rgb(t.subtle)).child(format!("Lv {level}")))),
                    ),
            )
            .children(extra)
            .child(self.render_grid(format!("{prefix}-board"), &view.board))
            .child(
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
            )
            .when(stash_open, |section| section.child(self.render_grid(format!("{prefix}-stash"), &view.stash)))
            .child(self.render_skills(prefix, &view.skills))
    }

    /// Ten empty slots, with the grid's items on them.
    pub(super) fn render_grid(&self, id: String, grid: &Grid) -> Div {
        let t = self.theme;
        let slots = f32::from(SLOTS);
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .relative()
                    .w_full()
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
            )
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

    fn render_skills(&self, prefix: &'static str, skills: &[SkillChip]) -> Div {
        let t = self.theme;
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1p5()
            .text_xs()
            .child(div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child("SKILLS"))
            .children(skills.iter().enumerate().map(|(index, skill)| {
                let mut lines = vec![skill.name.clone()];
                lines.extend(skill.tooltip.iter().cloned());
                div()
                    .id(SharedString::from(format!("{prefix}-skill-{index}")))
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(tier_colour(t, skill.tier)))
                    .bg(rgb(t.highlight_low))
                    .text_color(rgb(t.text))
                    .tooltip(widgets::hint(t, lines))
                    .child(skill.name.clone())
            }))
            .when(skills.is_empty(), |row| row.child(div().text_color(rgb(t.muted)).child("none")))
    }
}
