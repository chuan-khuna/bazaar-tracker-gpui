//! The settings overlay, in the middle of the window. It is empty for now.

use gpui::{Context, FontWeight, div, prelude::*, px, rgb};

use crate::ui::TrackerApp;
use crate::ui::widgets::{icon_button, panel};

impl TrackerApp {
    pub(in crate::ui) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div().absolute().inset_0().flex().items_center().justify_center().child(
            panel(t, "settings").w(px(480.)).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(40.))
                    .px_3()
                    .border_b_1()
                    .border_color(rgb(t.overlay))
                    .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(rgb(t.heading)).child("Settings"))
                    .child(icon_button(t, "close-settings", div().text_sm().text_color(rgb(t.subtle)).child("✕")).on_click(cx.listener(|this, _, _, cx| this.dismiss(cx)))),
            )
            .child(div().p_4().h(px(160.)).text_sm().text_color(rgb(t.muted)).child("There are no settings to change yet.")),
        )
    }
}
