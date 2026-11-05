//! The settings overlay, in the middle of the window. It holds the Card size section, a choice of
//! how big cards are drawn, the Any shop odds section, the Widget section (the pop-out's opacity
//! and frost), the Card art section: what the art folder has, and a button that extracts the art
//! from the game, the Item odds section, and the Data section, whose button clears the saved data.

use gpui::{Context, FontWeight, div, prelude::*, px, relative, rgb};
use gpui_base::Button;

use crate::shell::settings::CardSize;
use crate::overlays::Overlay;
use crate::overlays::settings_overlay::{CLEAR_WORD, SettingsOverlay};
use crate::theme::{self, Theme};
use crate::ui::TrackerApp;
use crate::ui::widgets::{icon_button, panel, scrollbar};

/// The opacities the Widget setting offers for the pop-out, in percent.
const WIDGET_OPACITIES: [u8; 5] = [20, 40, 60, 80, 100];

/// The frosts the Widget setting offers for the pop-out, in percent; 0 is no blur.
const WIDGET_FROSTS: [u8; 5] = [0, 25, 50, 75, 100];

impl TrackerApp {
    pub(in crate::ui) fn render_settings(&self, overlay: &SettingsOverlay, cx: &mut Context<Self>) -> impl IntoElement {
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
                                .child(self.render_data_setting(overlay, cx)),
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

    /// Widget: how opaque the pop-out over the game is, how frosted, and its theme.
    fn render_widget_setting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let look = self.settings.get().pop_out.clone();
        let opacity = WIDGET_OPACITIES.map(|percent| {
            self.filter_button(format!("widget-opacity-{percent}"), format!("{percent}%"), look.opacity == percent).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| settings.pop_out.set_opacity(percent), cx);
                cx.notify();
            }))
        });
        let frost = WIDGET_FROSTS.map(|percent| {
            let text = if percent == 0 { "Off".to_owned() } else { format!("{percent}%") };
            self.filter_button(format!("widget-frost-{percent}"), text, look.frost == percent).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| settings.pop_out.set_frost(percent), cx);
                cx.notify();
            }))
        });
        // "Same as app", then every theme.
        let themes = std::iter::once(("", "Same as app")).chain(theme::THEMES.iter().map(|theme| (theme.name, theme.name))).map(|(name, text)| {
            self.filter_button(format!("widget-theme-{text}"), text.to_owned(), look.theme == name).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| settings.pop_out.theme = name.to_owned(), cx);
                cx.notify();
            }))
        });
        let label = |text: &'static str| div().flex_none().w(px(64.)).text_xs().text_color(rgb(t.muted)).child(text);
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
            .child(div().flex().items_start().gap_1().child(label("Theme").pt_1()).child(div().flex().flex_wrap().gap_1().children(themes)))
            .child(div().text_xs().text_color(rgb(t.muted)).child(
                "How the pop-out over the game looks: how much of the game shows through it, how frosted that glass is (Off: not blurred), and its theme: a dark one reads best over the game.",
            ))
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

    /// Data: the button that deletes the saved runs and odds, the card art, or both. Clicking it
    /// asks what to clear and for [`CLEAR_WORD`] to be typed; Clear (or Enter) then clears, and
    /// Cancel (or Escape) stops asking.
    fn render_data_setting(&self, overlay: &SettingsOverlay, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let section = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .border_t_1()
            .border_color(rgb(t.overlay))
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child("DATA"));
        let note = div()
            .text_xs()
            .text_color(rgb(t.muted))
            .child("Deletes what the app has saved in its data folder, keeping the settings. The built-in shop odds come back; extract the card art again to see pictures.");
        let Some(ask) = overlay.clear.as_ref() else {
            let ask = data_button(t, "clear-data", "Clear data", t.error, true).on_click(cx.listener(|this, _, _, cx| {
                if let Some(Overlay::Settings(settings)) = this.overlay.as_mut() {
                    settings.ask_to_clear();
                }
                cx.notify();
            }));
            return section
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(div().min_w_0().text_sm().text_color(rgb(t.text)).child("Runs, shop and item odds, and card art."))
                        .child(ask),
                )
                .child(note);
        };
        let confirmed = ask.confirmed();
        let typed = ask.typed.as_str();
        let (database, art) = (ask.database, ask.art);
        // What to clear: each a toggle, both on at first.
        let picks = [("Database (runs and odds)", database, false), ("Card art", art, true)].map(|(label, on, is_art)| {
            self.filter_button(format!("clear-pick-{label}"), label.to_owned(), on).on_click(cx.listener(move |this, _, _, cx| {
                if let Some(Overlay::Settings(settings)) = this.overlay.as_mut()
                    && let Some(ask) = settings.clear.as_mut()
                {
                    let pick = if is_art { &mut ask.art } else { &mut ask.database };
                    *pick = !*pick;
                }
                cx.notify();
            }))
        });
        let warning = match (database, art) {
            (true, true) => "This deletes every run, the saved odds and the card art.",
            (true, false) => "This deletes every run and the saved odds.",
            (false, true) => "This deletes the card art.",
            (false, false) => "Pick what to clear.",
        };
        // What has been typed, with a caret, or what to type.
        let field = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .h(px(28.))
            .px_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(if confirmed { t.error } else { t.accent }))
            .text_sm()
            .map(|field| {
                if typed.is_empty() {
                    field.child(div().w(px(1.)).h(px(14.)).bg(rgb(t.text))).child(div().pl_1().text_color(rgb(t.muted)).child(format!("Type {CLEAR_WORD}")))
                } else {
                    field.child(div().text_color(rgb(t.text)).child(typed.to_owned())).child(div().w(px(1.)).h(px(14.)).bg(rgb(t.text)))
                }
            });
        let clear = data_button(t, "clear-data-confirm", "Clear", t.error, confirmed).when(confirmed, |button| button.on_click(cx.listener(move |this, _, _, cx| this.clear_data(database, art, cx))));
        let cancel = data_button(t, "clear-data-cancel", "Cancel", t.highlight_med, true).on_click(cx.listener(|this, _, _, cx| {
            if let Some(Overlay::Settings(settings)) = this.overlay.as_mut() {
                settings.stop_asking();
            }
            cx.notify();
        }));
        section
            .child(div().flex().flex_wrap().items_center().gap_1().children(picks))
            .child(div().text_sm().text_color(rgb(t.error)).child(format!("{warning} Type {CLEAR_WORD} to confirm.")))
            .child(div().flex().items_center().gap_2().child(field).child(clear).child(cancel))
            .child(note)
    }
}

/// A button with a word on it, outlined in `border`; dimmed and dead when not `enabled`.
fn data_button(t: &Theme, id: &'static str, label: &'static str, border: u32, enabled: bool) -> Button {
    let overlay = t.overlay;
    Button::new(id)
        .focusable(false)
        .tab_stop(false)
        .flex_none()
        .px_3()
        .h(px(28.))
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(rgb(if enabled { border } else { t.highlight_med }))
        .text_sm()
        .text_color(rgb(if enabled { t.text } else { t.muted }))
        .when(enabled, |button| button.cursor_pointer().hover(move |style| style.bg(rgb(overlay))))
        .child(label)
}
