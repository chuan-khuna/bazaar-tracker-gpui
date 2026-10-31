//! The main run panel in the middle, one section above another: Live, what the game shows now; then
//! the selected Run's header (and day buttons), its Economy, the gold over the Run; its Snapshot,
//! the board, stash and skills at the chosen moment, which folds away; and its Events.

use gpui::{Context, div, prelude::*, px, rgb};

use crate::ui::TrackerApp;
use super::board::Section;
use crate::ui::widgets::island;
use crate::tracking::run::board;

impl TrackerApp {
    pub(in crate::ui) fn render_main_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let snapshot = match self.session.selected_run() {
            Some(run) => {
                let view = board::view(self.session.events(), self.session.choice(), self.session.is_live(&run.id), &self.names);
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .children(self.render_run_header(cx))
                    .children(self.render_economy())
                    .child(self.render_board_section(&view, Section::Snapshot, Vec::new(), cx))
                    .child(self.render_events(cx))
            }
            None => div().flex().flex_col().flex_none().gap_1p5().px_3().py_2().child(div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child("SNAPSHOT")).child(
                div().text_sm().text_color(rgb(t.muted)).child("Start a run in The Bazaar, or select one from the sidebar."),
            ),
        };
        island(t)
            .flex_1()
            .child(
                div()
                    .id("main-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(self.render_live_section(cx))
                    .child(div().flex_none().h(px(1.)).mx_3().bg(rgb(t.overlay)))
                    .child(snapshot),
            )
            .child(crate::ui::widgets::scrollbar(t, "main-scrollbar", &self.main_scroll))
    }
}
