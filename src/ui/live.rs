//! The Live island above the Run island: what the game shows now. In a shop, the shop's
//! items; at an encounter choice, the options; in a combat, the opponent's skills and board over
//! the player's own; always the player's own items.

use gpui::{Context, Div, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::run::board::{MarkSide, Section};
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
            // A border in the live colour says this is the game as it is now, not a saved moment.
            Live::View(view) => {
                let extra = self.live_extras(&view);
                let section = self.render_board_section(&view.mine, Section::Live { with_stash: !view.combat }, extra, cx);
                div().flex().flex_col().flex_none().m_2().rounded_lg().border_1().border_color(rgb(t.live)).child(section)
            }
        }
    }

    /// What is on offer, or the opponent: the rows between the section's header and the player's
    /// own items, ending with a rule.
    fn live_extras(&self, view: &LiveView) -> Vec<Div> {
        let t = self.theme;
        let heading = |text: &'static str| div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(text);
        let mut rows = Vec::new();
        if let Some(offered) = &view.offered {
            // On a row of ten slots like the board, each card taking its own size.
            let mut row = div().flex().flex_col().gap_1().child(heading(offered.title));
            if !offered.items.tiles.is_empty() {
                row = row.child(self.render_grid("live-offer".to_owned(), &offered.items, MarkSide::Below));
            }
            row = row.children(offered.cards.iter().map(|card| self.render_card(card)));
            rows.push(row);
        }
        // The opponent in a box of their own, skills over board as the game shows them.
        if let Some(opponent) = &view.opponent {
            rows.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t.highlight_med))
                    .child(heading("OPPONENT"))
                    .when(!opponent.skills.is_empty(), |column| column.child(self.render_skills("live-opponent", &opponent.skills)))
                    .child(self.render_grid("live-opponent".to_owned(), &opponent.board, MarkSide::Above)),
            );
        }
        if !rows.is_empty() {
            rows.push(div().flex_none().h(px(1.)).my_1().bg(rgb(t.overlay)));
        }
        rows
    }
}
