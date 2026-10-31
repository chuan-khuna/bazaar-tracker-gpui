//! The settings overlay, in the middle of the window. It holds the Card art section: what the art
//! folder has, and a button that extracts the art from the game.

use gpui::{Context, FontWeight, div, prelude::*, px, rgb};
use gpui_base::Button;

use crate::ui::TrackerApp;
use crate::ui::widgets::{icon_button, panel};

impl TrackerApp {
    pub(in crate::ui) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div().absolute().inset_0().flex().items_center().justify_center().child(
            panel(t, "settings")
                .w(px(480.))
                .child(
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
                .child(self.render_art_setting(cx)),
        )
    }

    /// Card art: what is there now, or how far a run has got, and the button that starts one.
    fn render_art_setting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let running = self.art_run.is_some();
        let state = match (&self.art_run, &self.art) {
            (Some(progress), _) => progress.describe(),
            (None, Some(info)) => format!("{} cards have art · extracted {}", info.cards, info.date()),
            (None, None) => "Not extracted yet.".to_owned(),
        };
        let label = match (running, &self.art) {
            (true, _) => "Extracting…",
            (false, Some(_)) => "Update art",
            (false, None) => "Extract art",
        };
        let (accent, overlay) = (t.accent, t.overlay);
        let button = Button::new("extract-art")
            .focusable(false)
            .tab_stop(false)
            .flex_none()
            .px_3()
            .h(px(28.))
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(if running { t.highlight_med } else { accent }))
            .text_sm()
            .text_color(rgb(if running { t.muted } else { t.text }))
            .when(!running, |button| button.cursor_pointer().hover(move |style| style.bg(rgb(overlay))).on_click(cx.listener(|this, _, _, cx| this.extract_art(cx))))
            .child(label);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child("CARD ART"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(div().min_w_0().text_sm().text_color(rgb(t.text)).child(state))
                    .child(button),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(t.muted))
                    .child("Copies the card pictures from your copy of The Bazaar into the data folder, for this app only. Run it again after a game patch."),
            )
    }
}
