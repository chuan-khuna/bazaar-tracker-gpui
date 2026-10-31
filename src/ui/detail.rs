//! The detail panel on the right. It shows the selected event; for now, only its empty state.

use gpui::{Pixels, prelude::*, px};

use super::TrackerApp;
use super::widgets::{empty_state, heading, island};

/// The detail panel's width.
pub const WIDTH: Pixels = px(300.);

impl TrackerApp {
    pub(super) fn render_detail(&self) -> impl IntoElement {
        let t = self.theme;
        island(t).flex_none().w(WIDTH).child(heading(t, "DETAIL")).child(empty_state(t, "Select an event to see its details."))
    }
}
