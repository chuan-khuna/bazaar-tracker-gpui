//! The Live section at the top of the main island: what the game shows now. In a shop, the shop's
//! items; at an encounter choice, the options; in a combat, the opponent's board; always the
//! player's own items.

use gpui::{Context, Div, div, prelude::*, rgb};

use super::TrackerApp;
use crate::tracking::live::{Live, LiveView};
use crate::tracking::status::Connection;

impl TrackerApp {
    pub(super) fn render_live_section(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let attached = self.status.connection == Connection::Tracking;
        match self.live.view(attached, self.session.live().is_some(), &self.names) {
            Live::Placeholder(text) => div()
                .flex()
                .flex_col()
                .flex_none()
                .gap_1p5()
                .px_3()
                .py_2()
                .child(div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child("LIVE"))
                .child(div().text_sm().text_color(rgb(t.muted)).child(text)),
            Live::View(view) => {
                let extra = self.live_extras(&view);
                self.render_board_section(&view.mine, "live", "LIVE", extra, cx)
            }
        }
    }

    /// What is on offer, and the opponent's board: the rows between the section's header and the
    /// player's own items.
    fn live_extras(&self, view: &LiveView) -> Vec<Div> {
        let t = self.theme;
        let heading = |text: &'static str| div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(text);
        let mut rows = Vec::new();
        if let Some(offered) = &view.offered {
            let mut row = div().flex().flex_col().gap_1().child(heading(offered.title));
            if !offered.items.tiles.is_empty() {
                row = row.child(self.render_grid("live-offer".to_owned(), &offered.items));
            }
            row = row.children(offered.cards.iter().map(|card| self.render_card(card)));
            rows.push(row);
        }
        if let Some(opponent) = &view.opponent {
            rows.push(div().flex().flex_col().gap_1().child(heading("OPPONENT")).child(self.render_grid("live-opponent".to_owned(), opponent)));
        }
        rows
    }
}
