//! The main run panel in the middle: the selected Run's header, its board, stash and skills at the
//! chosen moment, and its Events.

use gpui::{Context, prelude::*};

use super::TrackerApp;
use super::widgets::{empty_state, island};
use crate::tracking::board;

impl TrackerApp {
    pub(super) fn render_main_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let Some(run) = self.session.selected_run() else {
            return island(t).flex_1().child(empty_state(t, "Start a run in The Bazaar, or select one from the sidebar."));
        };
        let view = board::view(self.session.events(), self.session.selected_event(), self.session.is_live(&run.id), &self.names);
        island(t).flex_1().children(self.render_run_header()).child(self.render_board_section(&view, cx)).child(self.render_events(cx))
    }
}
