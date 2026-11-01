//! Shop odds: the column beside the Economy (the Run's day and the two after it), and the Shop odds
//! widget's pop-up (every day, where the odds came from, and the buttons that replace them).

use gpui::{Context, Div, FontWeight, SharedString, div, prelude::*, px, relative, rgb};
use gpui_base::Button;

use crate::theme::Theme;
use crate::tracking::shop_odds::{self, OddsRow};
use crate::ui::TrackerApp;
use crate::ui::widgets::{self, icon_button, panel};

/// The share of the Economy's row the column beside it takes; the Economy takes the rest.
pub const COLUMN_SHARE: f32 = 0.3;

/// The least width of that column, so its percentages stay on one line.
const COLUMN_MIN_WIDTH: f32 = 180.;

/// What the pop-up says under its buttons: how the last refresh or import went.
#[derive(Clone, Debug)]
pub struct OddsMessage {
    pub text: SharedString,
    pub error: bool,
}

/// The tiers' colours, in the order an [`OddsRow`] holds them.
fn tier_colours(t: &Theme) -> [u32; 4] {
    [t.bronze, t.silver, t.gold, t.diamond]
}

/// The width of a day's name, on the left of its row.
const LABEL_WIDTH: f32 = 44.;

/// One day: its name on the left, level with a bar split by tier; each tier's share under the bar.
fn odds_row(t: &Theme, row: &OddsRow) -> Div {
    let name = div()
        .flex_none()
        .w(px(LABEL_WIDTH))
        .text_xs()
        .font_weight(if row.today { FontWeight::BOLD } else { FontWeight::NORMAL })
        .text_color(rgb(if row.today { t.heading } else { t.text }))
        .child(format!("Day {}", row.day));
    let borrowed = row.borrowed_from.map(|from| div().text_size(px(9.)).text_color(rgb(t.muted)).child(format!("as D{from}")));
    let line = |left: Div, right: Div| div().flex().items_center().gap_2().child(left).child(right);
    match row.tiers {
        None => div().flex().flex_col().child(line(name, div().flex_1().text_xs().text_color(rgb(t.muted)).child("Not enough data"))).children(borrowed),
        Some(tiers) => {
            let colours = tier_colours(t);
            let total: f32 = tiers.iter().sum::<f32>().max(1.);
            let bar = div().flex().flex_1().min_w_0().h(px(6.)).rounded_full().overflow_hidden().bg(rgb(t.overlay)).children(
                tiers.iter().zip(colours).filter(|(share, _)| **share > 0.).map(|(share, colour)| div().h_full().w(relative(share / total)).bg(rgb(colour))),
            );
            let shares = div().flex().flex_1().min_w_0().gap_2().children(tiers.iter().zip(colours).map(|(share, colour)| {
                div()
                    .flex()
                    .items_center()
                    .gap_0p5()
                    .child(div().size(px(5.)).rounded_full().bg(rgb(colour)))
                    .child(div().text_size(px(10.)).text_color(rgb(if *share > 0. { t.subtle } else { t.muted })).child(shop_odds::percent(*share)))
            }));
            div()
                .flex()
                .flex_col()
                .child(line(name, bar))
                .child(line(div().flex_none().w(px(LABEL_WIDTH)).children(borrowed), shares))
        }
    }
}

impl TrackerApp {
    /// The day the shop odds beside the Economy start from: the chosen day, or the Run's own.
    fn odds_day(&self) -> u32 {
        let run_day = self.session.selected_run().and_then(|run| run.day);
        self.session.selected_day().or(run_day).unwrap_or(1).max(1) as u32
    }

    /// The SHOP ODDS column beside the Economy.
    /// A click opens the Shop odds pop-up.
    pub(in crate::ui) fn render_upcoming_odds(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let overlay = t.overlay;
        let shown = shop_odds::upcoming(&self.shop_odds.odds, self.odds_day());
        div()
            .id("upcoming-odds")
            .flex()
            .flex_col()
            .flex_none()
            .w(relative(COLUMN_SHARE))
            .min_w(px(COLUMN_MIN_WIDTH))
            .gap_1p5()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(|this, _, _, cx| this.open_shop_odds(cx)))
            .child(widgets::section_header([widgets::section_title(t, "SHOP ODDS")], []))
            .child(div().flex().flex_col().gap_2p5().children(shown.rows.iter().map(|row| odds_row(t, row))))
            .children(shown.note.map(|note| div().text_size(px(10.)).text_color(rgb(t.muted)).child(note)))
    }

    /// The Shop odds pop-up, in the middle of the window.
    pub(in crate::ui) fn render_shop_odds_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let busy = self.shop_odds_busy;
        let rows = shop_odds::all_days(&self.shop_odds.odds);
        div().absolute().inset_0().flex().items_center().justify_center().child(
            panel(t, "shop-odds")
                .w(px(460.))
                .max_h(relative(0.9))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_between()
                        .h(px(40.))
                        .px_3()
                        .border_b_1()
                        .border_color(rgb(t.overlay))
                        .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(rgb(t.heading)).child("Shop odds"))
                        .child(icon_button(t, "close-shop-odds", div().text_sm().text_color(rgb(t.subtle)).child("✕")).on_click(cx.listener(|this, _, _, cx| this.dismiss(cx)))),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .gap_2()
                        .p_4()
                        .border_b_1()
                        .border_color(rgb(t.overlay))
                        .child(div().text_xs().text_color(rgb(t.muted)).child("The tier of each item offered at merchants, by day, from Bazaar DB's community-tracked runs."))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().flex_1().min_w_0().text_sm().text_color(rgb(t.text)).child(shop_odds::source(&self.shop_odds)))
                                .child(text_button(t, "refresh-shop-odds", if busy { "Refreshing…" } else { "Refresh" }, !busy).when(!busy, |button| button.on_click(cx.listener(|this, _, _, cx| this.refresh_shop_odds(cx)))))
                                .child(text_button(t, "import-shop-odds", "Import JSON…", !busy).when(!busy, |button| button.on_click(cx.listener(|this, _, _, cx| this.import_shop_odds(cx))))),
                        )
                        .children(self.shop_odds_message.as_ref().map(|message| div().text_xs().text_color(rgb(if message.error { t.error } else { t.success })).child(message.text.clone()))),
                )
                .child(
                    div()
                        .id("shop-odds-days")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .gap_2()
                        .p_4()
                        .overflow_y_scroll()
                        .map(|list| if rows.is_empty() { list.child(div().text_sm().text_color(rgb(t.muted)).child("No shop odds saved.")) } else { list.children(rows.iter().map(|row| odds_row(t, row))) }),
                ),
        )
    }
}

/// A button with a word on it, outlined in the accent colour; dimmed and dead when not `enabled`.
fn text_button(t: &Theme, id: &'static str, label: &'static str, enabled: bool) -> Button {
    let overlay = t.overlay;
    Button::new(id)
        .focusable(false)
        .tab_stop(false)
        .flex_none()
        .px_3()
        .h(px(28.))
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if enabled { t.accent } else { t.highlight_med }))
        .text_sm()
        .text_color(rgb(if enabled { t.text } else { t.muted }))
        .when(enabled, |button| button.cursor_pointer().hover(move |style| style.bg(rgb(overlay))))
        .child(label)
}
