//! The runs sidebar on the left, and the strip beside it that resizes it.

use gpui::{Context, MouseButton, Pixels, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::widgets::{empty_state, heading, island};
use crate::shell::settings::clamp_sidebar_width;

/// The space between islands, which is also the handle that resizes the sidebar.
pub const GAP: Pixels = px(8.);
/// The row of islands has this much padding on its left.
const ROW_PADDING: f32 = 8.;

impl TrackerApp {
    /// The strip between the sidebar and the main panel: drag it to change the sidebar's width.
    pub(super) fn render_sidebar_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .id("sidebar-handle")
            .flex_none()
            .w(GAP)
            .h_full()
            .cursor_col_resize()
            .when(self.resizing_sidebar, |handle| handle.child(div().mx_auto().w(px(2.)).h_full().bg(rgb(t.accent))))
            .hover(move |style| style.bg(rgb(t.overlay)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.resizing_sidebar = true;
                    cx.notify();
                }),
            )
    }

    /// Follow the pointer while the handle is held. The width is saved when the button is released.
    pub(super) fn drag_sidebar(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if self.resizing_sidebar {
            // The sidebar's width includes the gap beside it, and the row has padding on the left.
            let width = (f32::from(x) - ROW_PADDING + f32::from(GAP) / 2.).round().max(0.) as u32;
            self.sidebar_width = clamp_sidebar_width(width);
            cx.notify();
        }
    }

    pub(super) fn end_sidebar_drag(&mut self, cx: &mut Context<Self>) {
        if self.resizing_sidebar {
            self.resizing_sidebar = false;
            self.save_sidebar_width();
            cx.notify();
        }
    }

    pub(super) fn render_sidebar(&self) -> impl IntoElement {
        let t = self.theme;
        // An island: the width includes the gap beside it.
        island(t)
            .flex_none()
            .w(px(self.sidebar_width as f32) - GAP)
            .child(heading(t, "RUNS"))
            .child(empty_state(t, "No runs yet — start a run in The Bazaar and it will appear here."))
    }
}
