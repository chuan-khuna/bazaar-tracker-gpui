//! The settings overlay, in the middle of the window. It holds the Card size section, a choice of
//! how big cards are drawn, the Any shop odds section, the Widget section (the pop-out's opacity
//! and frost), the Card art section: what the art folder has, and a button that extracts the art
//! from the game, the Item odds section, and the Data section, whose button clears the saved data.

use gpui::{Context, FontWeight, div, prelude::*, px, relative, rgb};
use gpui_base::Button;

use crate::shell::settings::CardSize;
use crate::ui::TrackerApp;
use crate::ui::widgets::{icon_button, panel, scrollbar};

/// The opacities the Widget setting offers for the pop-out, in percent.
const WIDGET_OPACITIES: [u8; 5] = [20, 40, 60, 80, 100];

impl TrackerApp {
    pub(in crate::ui) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div().absolute().inset_0().flex().items_center().justify_center().child(
            panel(t, "settings")
                .w(px(480.))
                // No taller than the window allows; the sections scroll under the title.
                .max_h(relative(0.9))
                .overflow_hidden()
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
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .child(
                            div()
                                .id("settings-body")
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .track_scroll(&self.settings_scroll)
                                .child(self.render_card_size_setting(cx))
                                .child(self.render_any_shop_setting(cx))
                                .child(self.render_widget_setting(cx))
                                .child(self.render_art_setting(cx))
                                .child(self.render_odds_load_setting(cx))
                                .child(self.render_data_setting(cx)),
                        )
                        .child(scrollbar(t, "settings-scrollbar", &self.settings_scroll)),
                ),
        )
    }

    /// Card size: Small, Medium or Large, for item cards and skill icons. A click saves it.
    fn render_card_size_setting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let current = self.settings.get().card_size;
        let buttons = CardSize::ALL.map(|size| {
            self.filter_button(format!("card-size-{}", size.label()), size.label().to_owned(), size == current)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.change_settings(|settings| settings.card_size = size, cx);
                    cx.notify();
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .border_b_1()
            .border_color(rgb(t.overlay))
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child("CARD SIZE"))
            .child(div().flex().items_center().gap_1().children(buttons))
            .child(div().text_xs().text_color(rgb(t.muted)).child("How big item cards and skill icons are drawn."))
    }

    /// Any shop odds: whether item odds at an encounter choice end with the chance at any shop.
    fn render_any_shop_setting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let shown = self.settings.get().show_any_shop_odds;
        let buttons = [(false, "Hide"), (true, "Show")].map(|(value, label)| {
            self.filter_button(format!("any-shop-{label}"), label.to_owned(), value == shown).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| settings.show_any_shop_odds = value, cx);
                cx.notify();
            }))
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .border_b_1()
            .border_color(rgb(t.overlay))
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child("ANY SHOP ODDS"))
            .child(div().flex().items_center().gap_1().children(buttons))
            .child(div().text_xs().text_color(rgb(t.muted)).child("Whether item odds at an encounter choice end with the chance at any shop."))
    }

    /// Widget: how opaque the pop-out over the game is, and whether it is frosted glass.
    fn render_widget_setting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let look = self.settings.get().pop_out;
        let opacity = WIDGET_OPACITIES.map(|percent| {
            self.filter_button(format!("widget-opacity-{percent}"), format!("{percent}%"), look.opacity == percent).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| settings.pop_out.set_opacity(percent), cx);
                cx.notify();
            }))
        });
        let frost = [(false, "Off"), (true, "On")].map(|(value, label)| {
            self.filter_button(format!("widget-frost-{label}"), label.to_owned(), look.frost == value).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| settings.pop_out.frost = value, cx);
                cx.notify();
            }))
        });
        let label = |text: &'static str| div().w(px(64.)).text_xs().text_color(rgb(t.muted)).child(text);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .border_b_1()
            .border_color(rgb(t.overlay))
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child("WIDGET"))
            .child(div().flex().items_center().gap_1().child(label("Opacity")).children(opacity))
            .child(div().flex().items_center().gap_1().child(label("Frost")).children(frost))
            .child(div().text_xs().text_color(rgb(t.muted)).child("How the pop-out over the game looks: how much of the game shows through it, and whether that is blurred."))
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

    /// Data: the button that deletes the saved runs, odds and card art. The first click asks; the
    /// second clears.
    fn render_data_setting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let asking = self.clearing_data;
        let overlay = t.overlay;
        let button = Button::new("clear-data")
            .focusable(false)
            .tab_stop(false)
            .flex_none()
            .px_3()
            .h(px(28.))
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(t.error))
            .text_sm()
            .text_color(rgb(if asking { t.error } else { t.text }))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(|this, _, _, cx| {
                if this.clearing_data {
                    this.clear_data(cx);
                } else {
                    this.clearing_data = true;
                    cx.notify();
                }
            }))
            .child(if asking { "Click again to clear" } else { "Clear data" });
        let state = if asking { "This deletes every run, the saved odds and the card art." } else { "Runs, shop and item odds, and card art." };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .border_t_1()
            .border_color(rgb(t.overlay))
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child("DATA"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(div().min_w_0().text_sm().text_color(rgb(if asking { t.error } else { t.text })).child(state))
                    .child(button),
            )
            .child(div().text_xs().text_color(rgb(t.muted)).child("Deletes what the app has saved in its data folder, keeping the settings. The built-in shop odds come back; extract the card art again to see pictures."))
    }
}
