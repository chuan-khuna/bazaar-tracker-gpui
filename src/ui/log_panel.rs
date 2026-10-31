//! The log panel under the main island: a block of facts pinned at the top, and below it the
//! latest log rows, following the newest one unless the player has scrolled up.

use gpui::{Context, Div, MouseButton, Pixels, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::sidebar::GAP;
use super::status_bar;
use crate::shell::settings::clamp_log_height;
use crate::theme::Theme;
use crate::tracking::log::{Kind, LogRow};
use crate::tracking::when;

/// The space under the panels (the row of islands has `pb_1`) and the status bar's height.
const BELOW_PANELS: f32 = 4.;

fn kind_colour(t: &Theme, kind: Kind) -> u32 {
    match kind {
        Kind::Error => t.error,
        Kind::Warn => t.burn,
        Kind::Event => t.accent,
        Kind::Info => t.subtle,
    }
}

impl TrackerApp {
    /// The strip between the main island and the log panel: drag it to change the panel's height.
    pub(super) fn render_log_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .id("log-handle")
            .flex_none()
            .h(GAP)
            .w_full()
            .cursor_row_resize()
            .when(self.resizing_log, |handle| handle.child(div().my_auto().h(px(2.)).w_full().bg(rgb(t.accent))))
            .hover(move |style| style.bg(rgb(t.overlay)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.resizing_log = true;
                    cx.notify();
                }),
            )
    }

    /// Follow the pointer while the handle is held. The height is saved when the button is released.
    pub(super) fn drag_log(&mut self, y: Pixels, window_height: Pixels, cx: &mut Context<Self>) {
        if self.resizing_log {
            // The panel ends above the padding and the status bar; the handle's middle is under the pointer.
            let bottom = f32::from(window_height) - status_bar::HEIGHT_PX - BELOW_PANELS;
            let height = (bottom - f32::from(y) - f32::from(GAP) / 2.).round().max(0.) as u32;
            self.log_height = clamp_log_height(height);
            cx.notify();
        }
    }

    pub(super) fn end_log_drag(&mut self, cx: &mut Context<Self>) {
        if self.resizing_log {
            self.resizing_log = false;
            let height = self.log_height;
            // A failed save is logged; the height is still in use.
            if let Err(error) = self.settings.update(|settings| settings.window.set_log_height(height)) {
                tracing::error!("Could not save settings: {error}");
            }
            cx.notify();
        }
    }

    pub(super) fn render_log_panel(&self) -> impl IntoElement {
        let t = self.theme;
        let facts = self.details.lines();
        super::widgets::island(t)
            .flex_none()
            .w_full()
            .h(px(self.log_height as f32))
            .child(
                // Pinned: it does not scroll with the log.
                div()
                    .flex()
                    .flex_none()
                    .flex_wrap()
                    .gap_x_4()
                    .gap_y_0p5()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(t.overlay))
                    .text_xs()
                    .children(facts.into_iter().map(|(label, value)| div().flex().gap_1().child(div().text_color(rgb(t.muted)).child(label)).child(div().text_color(rgb(t.text)).child(value)))),
            )
            .child(
                div()
                    .id("log")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .py_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.log_scroll)
                    .children(self.log.rows().map(|row| log_row(t, row))),
            )
    }
}

fn log_row(t: &Theme, row: &LogRow) -> Div {
    let text = match row.kind {
        Kind::Error => t.error,
        Kind::Warn => t.burn,
        Kind::Info | Kind::Event => t.text,
    };
    div()
        .flex()
        .flex_none()
        .gap_3()
        .text_xs()
        .child(div().flex_none().text_color(rgb(t.muted)).child(when::hms(row.time_ms)))
        .child(div().flex_none().w(px(40.)).text_color(rgb(kind_colour(t, row.kind))).child(row.kind.label()))
        .child(div().flex_1().min_w_0().text_color(rgb(text)).child(row.text.clone()))
}
