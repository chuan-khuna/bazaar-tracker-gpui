//! The runs sidebar on the left, with the widgets along its bottom, and the strip beside it that
//! resizes it.

use bazaar_tracker::run::Outcome;
use gpui::{AnyElement, Context, Div, MouseButton, Pixels, SharedString, Stateful, div, prelude::*, px, rgb};

use crate::ui::TrackerApp;
use crate::ui::widgets::{self, IconName, Tone, empty_state, heading, island};
use crate::tracking::session::RunRow;
use crate::shell::settings::clamp_sidebar_width;

/// The space between islands, which is also the handle that resizes the sidebar.
pub const GAP: Pixels = px(8.);
/// The row of islands has this much padding on its left.
const ROW_PADDING: f32 = 8.;
/// The group name of a Run's row, so that its delete button shows while the pointer is over it.
const RUN_ROW: &str = "run-row";

impl TrackerApp {
    /// The strip between the sidebar and the main panel: drag it to change the sidebar's width.
    pub(in crate::ui) fn render_sidebar_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
    pub(in crate::ui) fn drag_sidebar(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if self.resizing_sidebar {
            // The sidebar's width includes the gap beside it, and the row has padding on the left.
            let width = (f32::from(x) - ROW_PADDING + f32::from(GAP) / 2.).round().max(0.) as u32;
            self.sidebar_width = clamp_sidebar_width(width);
            cx.notify();
        }
    }

    pub(in crate::ui) fn end_sidebar_drag(&mut self, cx: &mut Context<Self>) {
        if self.resizing_sidebar {
            self.resizing_sidebar = false;
            self.save_sidebar_width();
            cx.notify();
        }
    }

    pub(in crate::ui) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let rows: Vec<_> = self.session.rows().into_iter().map(|row| self.render_run_row(row, cx)).collect();
        // An island: the width includes the gap beside it. The widgets sit along its bottom.
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
        .child(self.render_widgets(cx))
    }

    /// The WIDGETS section at the bottom of the sidebar: a button for each widget, which opens its pop-up.
    fn render_widgets(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        // How many items are tracked in the Run being played or looked at.
        let tracked = match self.tracked().len() {
            0 => String::new(),
            count => count.to_string(),
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .border_t_1()
            .border_color(rgb(t.overlay))
            .child(heading(t, "WIDGETS"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .px_1p5()
                    .pb_1p5()
                    .child(widgets::menu_item(t, "widget-shop-odds", "Shop odds", div()).on_click(cx.listener(|this, _, _, cx| this.open_shop_odds(cx))))
                    .child(widgets::menu_item(t, "widget-tracked-items", "Tracked items", div().text_xs().text_color(rgb(t.muted)).child(tracked)).on_click(cx.listener(|this, _, _, cx| this.open_tracked_items(cx)))),
            )
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
        // The live Run cannot be deleted: the tracker is still saving to it.
        let delete = if row.live { None } else { Some(self.render_delete_button(&row.id, cx)) };
        div()
            .id(SharedString::from(format!("run-{}", row.id)))
            .group(RUN_ROW)
            .flex()
            .items_center()
            .flex_none()
            .gap_1()
            .px_2()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .when(row.selected, |div| div.bg(rgb(overlay)))
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(move |this, _, _, cx| this.select_run(&id, cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    // The hero, with the record and the day as tags; under it, when the Run started.
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1p5()
                            .child(div().text_sm().text_color(rgb(t.text)).child(row.hero))
                            .child(widgets::badge(row.record, record_colour, Tone::Soft, t))
                            .children(row.day.map(|day| widgets::badge(day, t.subtle, Tone::Soft, t)))
                            .when(row.live, |line| line.child(widgets::live_badge(t))),
                    )
                    .child(div().text_xs().text_color(rgb(t.muted)).child(row.started)),
            )
            .children(delete)
    }

    /// The button that deletes a Run: a trash can, shown while the pointer is over the row, which
    /// asks "Delete?" when clicked and deletes the Run when clicked again.
    fn render_delete_button(&self, run_id: &str, cx: &mut Context<Self>) -> AnyElement {
        let t = self.theme;
        let asking = self.deleting_run.as_deref() == Some(run_id);
        let id = run_id.to_owned();
        let button_id = SharedString::from(format!("delete-run-{run_id}"));
        let on_click = cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.delete_run(&id, cx);
        });
        if asking {
            div()
                .id(button_id)
                .flex_none()
                .px_1p5()
                .py_0p5()
                .rounded_md()
                .bg(rgb(t.error))
                .text_color(rgb(t.background))
                .text_xs()
                .font_weight(gpui::FontWeight::BOLD)
                .cursor_pointer()
                .on_click(on_click)
                .child("Delete?")
                .into_any_element()
        } else {
            div()
                .flex_none()
                .invisible()
                .group_hover(RUN_ROW, |style| style.visible())
                .child(widgets::icon_button(t, button_id, widgets::icon(IconName::Trash, t.subtle)).on_click(on_click))
                .into_any_element()
        }
    }
}
