//! The flat bar along the bottom of the window: the sidebar button and the tracker's connection
//! state on the left, an error (kept until dismissed) and a short notice beside them, and the
//! detail and log buttons on the right.

use gpui::{Context, Pixels, div, prelude::*, px, rgb};

use crate::ui::TrackerApp;
use crate::ui::widgets::{self, IconName};
use crate::tracking::status::Connection;

const HEIGHT: Pixels = px(HEIGHT_PX);

/// The status bar's height, in pixels.
pub const HEIGHT_PX: f32 = 28.;

impl TrackerApp {
    pub(in crate::ui) fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let window = &self.settings.get().window;
        let (sidebar_open, detail_open, log_open) = (window.sidebar_open, self.detail_open, window.log_open);
        let connection = self.status.connection;
        let dot = match connection {
            Connection::NotTracking | Connection::Waiting => t.muted,
            Connection::Tracking => t.success,
        };

        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .w_full()
            .h(HEIGHT)
            .px_1()
            .bg(rgb(t.background))
            .text_xs()
            .text_color(rgb(t.subtle))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        widgets::icon_button(t, "toggle-sidebar", widgets::icon(IconName::PanelLeft, if sidebar_open { t.text } else { t.subtle }))
                            .when(sidebar_open, |button| button.bg(rgb(t.overlay)))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    )
                    .child(div().flex().items_center().gap_1p5().child(div().size(px(7.)).rounded_full().bg(rgb(dot))).child(connection.label()))
                    // An error stays until it is dismissed.
                    .children(self.error.clone().map(|error| {
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_color(rgb(t.error))
                            .child(error)
                            .child(widgets::icon_button(t, "dismiss-error", div().text_color(rgb(t.error)).child("✕")).on_click(cx.listener(|this, _, _, cx| this.dismiss_error(cx))))
                    }))
                    // A notice hides by itself.
                    .children(self.notice.clone().map(|notice| div().text_color(rgb(t.text)).child(notice))),
            )
            // The two panels on the right, in the order they sit: detail, then log.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        widgets::icon_button(t, "toggle-detail", widgets::icon(IconName::PanelRight, if detail_open { t.text } else { t.subtle }))
                            .when(detail_open, |button| button.bg(rgb(t.overlay)))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_detail(cx))),
                    )
                    .child(
                        widgets::icon_button(t, "toggle-log", widgets::icon(IconName::SquareTerminal, if log_open { t.text } else { t.subtle }))
                            .when(log_open, |button| button.bg(rgb(t.overlay)))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_log(cx))),
                    )
                    .map(|buttons| self.web_button(buttons, cx)),
            )
    }

    /// The Bazaar DB debug panel's button, after the log's.
    #[cfg(windows)]
    fn web_button(&self, buttons: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let open = self.web.open;
        buttons.child(
            widgets::icon_button(t, "toggle-web", widgets::icon(IconName::Globe, if open { t.text } else { t.subtle }))
                .when(open, |button| button.bg(rgb(t.overlay)))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_web(cx))),
        )
    }

    #[cfg(not(windows))]
    fn web_button(&self, buttons: gpui::Div, _: &mut Context<Self>) -> gpui::Div {
        buttons
    }
}
