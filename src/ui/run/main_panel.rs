//! The middle column, as two islands one above the other. The Live island shows what the game shows
//! now; it is as tall as what it shows, up to a height the strip under it drags, and scrolls past
//! that. The Run island under it shows the selected Run: its header (and day buttons), its Economy,
//! the gold over the Run, with the shop odds of the next days beside it, and its Events. The Snapshot of the chosen moment is in the detail panel.

use gpui::{Context, MouseButton, Pixels, div, prelude::*, px, rgb};

use crate::ui::TrackerApp;
use crate::ui::frame::sidebar::GAP;
use crate::ui::widgets::{self, island, scrollbar};
use crate::shell::settings::clamp_live_height;

/// The least height the Run island keeps however far the Live island is dragged.
const RUN_MIN_HEIGHT: f32 = 120.;

impl TrackerApp {
    pub(in crate::ui) fn render_main_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let snapshot = match self.session.selected_run() {
            Some(_) => div().flex().flex_col().flex_none().children(self.render_run_header(cx)).child(self.render_economy_row(cx)).child(self.render_events(cx)),
            None => div().flex().flex_col().flex_none().gap_1p5().px_3().py_2().child(widgets::section_title(t, "RUN")).child(
                div().text_sm().text_color(rgb(t.muted)).child("Start a run in The Bazaar, or select one from the sidebar."),
            ),
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                island(t)
                    .flex_initial()
                    .h_auto()
                    .min_h_0()
                    .max_h(px(self.live_height as f32))
                    .child(div().id("live-scroll").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.live_scroll).child(self.render_live_section(cx)))
                    .child(scrollbar(t, "live-scrollbar", &self.live_scroll)),
            )
            .child(self.render_live_handle(cx))
            .child(
                island(t)
                    .flex_1()
                    .min_h(px(RUN_MIN_HEIGHT))
                    .child(div().id("main-scroll").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.main_scroll).child(snapshot))
                    .child(scrollbar(t, "main-scrollbar", &self.main_scroll)),
            )
    }

    /// The strip between the Live island and the Run island: drag it to change the most height the
    /// Live island takes.
    fn render_live_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .id("live-handle")
            .flex_none()
            .w_full()
            .h(GAP)
            .cursor_row_resize()
            .when(self.resizing_live.is_some(), |handle| handle.child(div().my_auto().h(px(2.)).w_full().bg(rgb(t.accent))))
            .hover(move |style| style.bg(rgb(t.overlay)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    // From the height the island has now, which is less than its most when what it
                    // shows is short.
                    let height = f32::from(this.live_scroll.bounds().size.height);
                    this.resizing_live = Some((f32::from(event.position.y), height));
                    cx.notify();
                }),
            )
    }

    /// Follow the pointer while the strip is held. The height is saved when the button is released.
    pub(in crate::ui) fn drag_live(&mut self, y: Pixels, cx: &mut Context<Self>) {
        if let Some((start_y, start_height)) = self.resizing_live {
            let height = (start_height + f32::from(y) - start_y).round().max(0.) as u32;
            self.live_height = clamp_live_height(height);
            cx.notify();
        }
    }

    pub(in crate::ui) fn end_live_drag(&mut self, cx: &mut Context<Self>) {
        if self.resizing_live.take().is_some() {
            let height = self.live_height;
            // A failed save is logged; the height is still in use.
            if let Err(error) = self.settings.update(|settings| settings.window.set_live_height(height)) {
                tracing::error!("Could not save settings: {error}");
            }
            cx.notify();
        }
    }
}
