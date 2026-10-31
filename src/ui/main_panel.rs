//! The main run panel in the middle, in two sections one above the other: Live, what the game shows
//! now, and Snapshot, the selected Run's board, stash and skills at the chosen moment with its
//! header (and day buttons) and its Events.
//!
//! The content has a least width, so that a board's ten slots stay readable; an island narrower than
//! that scrolls sideways.

use gpui::{Context, div, prelude::*, px, rgb};
use gpui_base::ScrollbarAxis;

use super::TrackerApp;
use super::widgets::island;
use crate::tracking::board;

/// The least width of the main island's content: ten slots of about 64 px, with the padding.
const MIN_CONTENT_WIDTH: f32 = 680.;

impl TrackerApp {
    pub(super) fn render_main_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let snapshot = match self.session.selected_run() {
            Some(run) => {
                let view = board::view(self.session.events(), self.session.choice(), self.session.is_live(&run.id), &self.names);
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .children(self.render_run_header(cx))
                    .child(self.render_board_section(&view, "snapshot", "SNAPSHOT", Vec::new(), true, cx))
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
                    .overflow_scroll()
                    .track_scroll(&self.main_scroll)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w_full()
                            .min_w(px(MIN_CONTENT_WIDTH))
                            .child(self.render_live_section(cx))
                            .child(div().flex_none().h(px(1.)).mx_3().bg(rgb(t.overlay)))
                            .child(snapshot),
                    ),
            )
            .child(super::widgets::scrollbar(t, "main-scrollbar", &self.main_scroll).axis(ScrollbarAxis::Both))
    }
}
