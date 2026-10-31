//! The log panel on the right: a block of facts pinned at the top, and below it the latest log
//! rows, following the newest one unless the player has scrolled up.

use gpui::{Context, Div, MouseButton, Pixels, div, list, prelude::*, px, rgb};

use super::TrackerApp;
use super::sidebar::GAP;
use crate::shell::settings::clamp_log_width;
use crate::theme::Theme;
use crate::tracking::log::{Kind, LogRow};
use crate::tracking::when;

/// The row of islands has this much padding on its right.
const ROW_PADDING: f32 = 8.;

/// The log's rows and facts are monospaced, so the times and labels line up and addresses and
/// paths read cleanly. Consolas ships with every Windows.
const MONO: &str = "Consolas";

fn kind_colour(t: &Theme, kind: Kind) -> u32 {
    match kind {
        Kind::Error => t.error,
        Kind::Warn => t.burn,
        Kind::Event => t.accent,
        Kind::Now => t.success,
        Kind::Info => t.subtle,
    }
}

impl TrackerApp {
    /// The strip on the log panel's left: drag it to change the panel's width.
    pub(super) fn render_log_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .id("log-handle")
            .flex_none()
            .w(GAP)
            .h_full()
            .cursor_col_resize()
            .when(self.resizing_log, |handle| handle.child(div().mx_auto().w(px(2.)).h_full().bg(rgb(t.accent))))
            .hover(move |style| style.bg(rgb(t.overlay)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.resizing_log = true;
                    cx.notify();
                }),
            )
    }

    /// Follow the pointer while the handle is held. The width is saved when the button is released.
    pub(super) fn drag_log(&mut self, x: Pixels, window_width: Pixels, cx: &mut Context<Self>) {
        if self.resizing_log {
            // The panel ends at the row's right padding; the handle's middle is under the pointer.
            let width = (f32::from(window_width) - ROW_PADDING - f32::from(x) - f32::from(GAP) / 2.).round().max(0.) as u32;
            self.log_width = clamp_log_width(width);
            cx.notify();
        }
    }

    pub(super) fn end_log_drag(&mut self, cx: &mut Context<Self>) {
        if self.resizing_log {
            self.resizing_log = false;
            let width = self.log_width;
            // A failed save is logged; the width is still in use.
            if let Err(error) = self.settings.update(|settings| settings.window.set_log_width(width)) {
                tracing::error!("Could not save settings: {error}");
            }
            cx.notify();
        }
    }

    pub(super) fn render_log_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let facts = self.details.lines();
        super::widgets::island(t)
            .flex_none()
            .w(px(self.log_width as f32))
            .h_full()
            .child(super::widgets::heading(t, "LOG"))
            .child(
                // Pinned: it does not scroll with the log.
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap_0p5()
                    .px_3()
                    .pb_2()
                    .border_b_1()
                    .border_color(rgb(t.overlay))
                    .text_xs()
                    .font_family(MONO)
                    .children(facts.into_iter().map(|(label, value)| {
                        div().flex().gap_2().child(div().flex_none().w(px(84.)).text_color(rgb(t.muted)).child(label)).child(div().flex_1().min_w_0().text_color(rgb(t.text)).child(value))
                    })),
            )
            // Only the rows in view are laid out and drawn.
            .child(
                list(
                    self.log_list.clone(),
                    cx.processor(|this, index: usize, _, _| match this.log.row(index) {
                        Some(row) => log_row(this.theme, row).into_any_element(),
                        None => div().into_any_element(),
                    }),
                )
                .flex_1()
                .min_h_0()
                .py_1(),
            )
            .child(super::widgets::scrollbar(t, "log-scrollbar", &self.log_list))
    }
}

fn log_row(t: &Theme, row: &LogRow) -> Div {
    let text = match row.kind {
        Kind::Error => t.error,
        Kind::Warn => t.burn,
        Kind::Info | Kind::Event | Kind::Now => t.text,
    };
    div()
        .flex()
        .flex_none()
        .gap_3()
        .px_2()
        .text_xs()
        .font_family(MONO)
        .child(div().flex_none().text_color(rgb(t.muted)).child(when::hms(row.time_ms)))
        .child(div().flex_none().w(px(40.)).text_color(rgb(kind_colour(t, row.kind))).child(row.kind.label()))
        .child(div().flex_1().min_w_0().text_color(rgb(text)).child(row.text.clone()))
}
