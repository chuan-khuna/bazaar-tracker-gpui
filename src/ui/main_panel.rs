//! The main run panel in the middle. It shows the selected run; for now, only its empty state.

use gpui::prelude::*;

use super::TrackerApp;
use super::widgets::{empty_state, island};

impl TrackerApp {
    pub(super) fn render_main_panel(&self) -> impl IntoElement {
        let t = self.theme;
        island(t).flex_1().child(empty_state(t, "Start a run in The Bazaar, or select one from the sidebar."))
    }
}
