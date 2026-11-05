//! The middle column, one island that scrolls as a whole: the Live section, what the game shows now,
//! then the selected Run's Economy, the gold over the Run, with the shop odds of the next days
//! beside it. The Run's header, Snapshot and Events are in the detail panel.

use gpui::{Context, div, prelude::*, rgb};

use crate::ui::TrackerApp;
use crate::ui::widgets::{self, island, scrollbar};

impl TrackerApp {
    pub(in crate::ui) fn render_main_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let economy = match self.session.selected_run() {
            Some(_) => div().flex().flex_col().flex_none().border_t_1().border_color(rgb(t.overlay)).child(self.render_economy_row(cx)),
            None => div().flex().flex_col().flex_none().gap_1p5().px_3().py_2().child(widgets::section_title(t, "RUN")).child(
                div().text_sm().text_color(rgb(t.muted)).child("Start a run in The Bazaar, or select one from the sidebar."),
            ),
        };
        div().flex().flex_col().flex_1().min_w_0().h_full().child(
            island(t)
                .flex_1()
                .min_h_0()
                .child(div().id("main-scroll").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.main_scroll).child(self.render_live_section(cx)).child(economy))
                .child(scrollbar(t, "main-scrollbar", &self.main_scroll)),
        )
    }
}
