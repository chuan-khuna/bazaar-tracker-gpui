//! The runs sidebar on the left, and the strip beside it that resizes it.

use bazaar_tracker::run::Outcome;
use gpui::{Context, Div, MouseButton, Pixels, SharedString, Stateful, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::widgets::{self, empty_state, heading, island};
use crate::tracking::session::RunRow;
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

    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let rows: Vec<_> = self.session.rows().into_iter().map(|row| self.render_run_row(row, cx)).collect();
        // An island: the width includes the gap beside it.
        island(t).flex_none().w(px(self.sidebar_width as f32) - GAP).child(heading(t, "RUNS")).map(|panel| {
            if rows.is_empty() {
                panel.child(empty_state(t, "No runs yet — start a run in The Bazaar and it will appear here."))
            } else {
                panel
                    .child(
                        div()
                            .id("runs")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .gap_0p5()
                            .px_1p5()
                            .pb_1p5()
                            .overflow_y_scroll()
                            .track_scroll(&self.runs_scroll)
                            .children(rows),
                    )
                    .child(widgets::scrollbar(t, "runs-scrollbar", &self.runs_scroll))
            }
        })
    }

    fn render_run_row(&self, row: RunRow, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let overlay = t.overlay;
        let record_colour = match row.outcome {
            Some(Outcome::Win) => t.win,
            Some(Outcome::Loss) => t.loss,
            _ => t.subtle,
        };
        let id = row.id.clone();
        div()
            .id(SharedString::from(format!("run-{}", row.id)))
            .flex()
            .flex_col()
            .flex_none()
            .gap_0p5()
            .px_2()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .when(row.selected, |div| div.bg(rgb(overlay)))
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(move |this, _, _, cx| this.select_run(&id, cx)))
            // The hero, with the record and the day as tags; under it, when the Run started.
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1p5()
                    .child(div().text_sm().text_color(rgb(t.text)).child(row.hero))
                    .child(widgets::tag(row.record, record_colour))
                    .children(row.day.map(|day| widgets::tag(day, t.subtle)))
                    .when(row.live, |line| line.child(widgets::chip(t, "LIVE", t.live))),
            )
            .child(div().text_xs().text_color(rgb(t.muted)).child(row.started))
    }
}
