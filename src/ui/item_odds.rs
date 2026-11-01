//! Item odds: the button on an item's tile (shown while it is hovered) that loads the item's odds
//! from Bazaar DB or, once they are saved, opens them; the pop-up with every merchant and day; and
//! at an encounter choice, the chances beside each merchant on offer.
//!
//! Loading goes through the Bazaar DB panel: its shop odds page asks Bazaar DB for the item (the
//! player picks it there the first time), and only a reply for the item asked about is saved.

use gpui::{Context, Div, FontWeight, SharedString, div, prelude::*, px, relative, rgb, rgba};

use crate::overlays::Overlay;
use crate::overlays::item_odds::ItemOddsOverlay;
use crate::tracking::item_odds::{self, Wanted};
use crate::tracking::live::LiveView;
use crate::tracking::run::board::Tile;
use crate::ui::TrackerApp;
use crate::ui::widgets::{self, IconName, icon_button, panel};

/// Whose odds an item's tile asks for: the hero playing, and the day whose column is marked.
#[derive(Clone, Debug)]
pub(in crate::ui) struct OddsContext {
    pub hero: String,
    pub day: Option<u32>,
}

impl OddsContext {
    /// For `hero` on `day`, when the hero is known.
    pub fn new(hero: Option<&str>, day: Option<i32>) -> Option<Self> {
        let hero = hero.filter(|hero| !hero.is_empty())?;
        Some(Self { hero: hero.to_owned(), day: day.and_then(|day| u32::try_from(day).ok()) })
    }
}

/// The group a tile's hover shows its odds button in.
pub(in crate::ui) const TILE_GROUP: &str = "tile";

/// The width of the merchant column of the pop-up's table.
const MERCHANT_WIDTH: f32 = 150.;

impl TrackerApp {
    /// The tile's odds button, at its top right, seen while the tile is hovered (and always while its
    /// odds are being loaded). An item with no card id has none.
    pub(in crate::ui) fn odds_button(&self, id: SharedString, tile: &Tile, odds: &OddsContext, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let t = self.theme;
        let wanted = Wanted { template_id: tile.template_id.clone()?, name: tile.name.clone(), hero: odds.hero.clone(), tier: tile.tier_name.clone() };
        let saved = self.item_odds.get(&wanted.template_id, &wanted.hero, &wanted.tier).is_some();
        let pending = self.item_odds_pending.as_ref() == Some(&wanted);
        let (icon, colour, hint) = match (saved, pending) {
            (_, true) => (IconName::Loader, t.subtle, "Waiting for Bazaar DB: pick the item on its page"),
            (true, false) => (IconName::ChartPie, t.accent, "Show item odds"),
            (false, false) => (IconName::Search, t.text, "Load item odds from Bazaar DB"),
        };
        let day = odds.day;
        Some(
            div()
                .id(id)
                .absolute()
                .top(px(2.))
                .right(px(2.))
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .bg(rgba(0x0000_00c0))
                .cursor_pointer()
                .when(!pending, |button| button.opacity(0.).group_hover(TILE_GROUP, |style| style.opacity(1.)))
                .tooltip(widgets::hint(t, vec![hint.to_owned()]))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.item_odds_clicked(wanted.clone(), day, cx);
                }))
                .child(widgets::icon(icon, colour).size(px(12.))),
        )
    }

    /// The tile's button: the saved odds if there are some, else load them.
    fn item_odds_clicked(&mut self, wanted: Wanted, day: Option<u32>, cx: &mut Context<Self>) {
        if self.item_odds.get(&wanted.template_id, &wanted.hero, &wanted.tier).is_some() {
            self.open(Overlay::ItemOdds(ItemOddsOverlay { item: wanted, day }), cx);
        } else {
            self.ask_item_odds(wanted, cx);
        }
    }

    /// Load `wanted`'s odds: open the Bazaar DB panel on the shop odds page, on the item if its
    /// Bazaar DB id is known (else the player picks it there), and wait for the reply.
    #[cfg(windows)]
    pub(in crate::ui) fn ask_item_odds(&mut self, wanted: Wanted, cx: &mut Context<Self>) {
        let url_id = self.item_odds.url_id(&wanted.template_id).map(str::to_owned);
        let page = bazaardb::webview::item_odds::page_for(url_id.as_deref(), &wanted.hero, &wanted.tier);
        tracing::info!("item odds: asking Bazaar DB for {} ({}, {})", wanted.name, wanted.hero, wanted.tier);
        if url_id.is_none() {
            self.show_notice(format!("Pick {} on Bazaar DB's page", wanted.name), cx);
        }
        self.item_odds_pending = Some(wanted);
        self.show_in_web(&page, cx);
        cx.notify();
    }

    #[cfg(not(windows))]
    pub(in crate::ui) fn ask_item_odds(&mut self, _wanted: Wanted, cx: &mut Context<Self>) {
        self.show_notice("Item odds load through the Bazaar DB panel, which is only on Windows", cx);
    }

    /// An item odds reply from the Bazaar DB panel. It is saved if it is for the item being waited
    /// for; returns whether it was.
    pub(in crate::ui) fn take_item_odds(&mut self, reply: &bazaardb::ItemOdds, cx: &mut Context<Self>) -> bool {
        let Some(wanted) = self.item_odds_pending.clone() else { return false };
        let known = self.item_odds.url_id(&wanted.template_id).map(str::to_owned);
        let accepted = match item_odds::accept(&wanted, reply, known.as_deref(), super::now_ms()) {
            Ok(accepted) => accepted,
            Err(why) => {
                // Not the item (the player may be looking at others): keep waiting.
                tracing::info!("item odds: {why}");
                self.show_notice(format!("Waiting for {}", wanted.name), cx);
                return false;
            }
        };
        if let Some(history) = self.history.as_deref()
            && let Err(error) = history.save_item_odds(&accepted.saved)
        {
            self.show_error(format!("Could not save the odds for {}: {error}", wanted.name), cx);
            return false;
        }
        let name = accepted.saved.item_name.clone();
        if let Err(why) = self.item_odds.insert(accepted.saved) {
            self.show_error(format!("Could not read the odds for {name}: {why}"), cx);
            return false;
        }
        self.item_odds_pending = None;
        match accepted.note {
            Some(note) => self.show_error(note, cx),
            None => self.show_notice(format!("Saved the odds for {name}"), cx),
        }
        true
    }

    /// The pop-up of one item's saved odds, in the middle of the window.
    pub(in crate::ui) fn render_item_odds_overlay(&self, overlay: &ItemOddsOverlay, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let item = &overlay.item;
        let entry = self.item_odds.get(&item.template_id, &item.hero, &item.tier);
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(40.))
            .px_3()
            .border_b_1()
            .border_color(rgb(t.overlay))
            .child(div().text_sm().font_weight(FontWeight::BOLD).text_color(rgb(t.heading)).child(format!("Item odds · {}", item.name)))
            .child(icon_button(t, "close-item-odds", div().text_sm().text_color(rgb(t.subtle)).child("✕")).on_click(cx.listener(|this, _, _, cx| this.dismiss(cx))));
        let refresh = {
            let wanted = item.clone();
            icon_button(t, "refresh-item-odds", widgets::icon(IconName::RefreshCw, t.subtle)).on_click(cx.listener(move |this, _, _, cx| {
                // The panel's web view steps aside while a pop-up is open.
                this.dismiss(cx);
                this.ask_item_odds(wanted.clone(), cx);
            }))
        };
        let body = match entry {
            None => div().p_4().text_sm().text_color(rgb(t.muted)).child("No odds saved for this item."),
            Some(entry) => {
                let view = item_odds::table_view(entry, overlay.day);
                let cell = |text: SharedString, today: bool| {
                    div().flex_1().min_w(px(40.)).px_1().py_0p5().text_right().rounded_sm().when(today, |cell| cell.bg(rgb(t.overlay))).child(text)
                };
                let head = div()
                    .flex()
                    .items_center()
                    .text_color(rgb(t.muted))
                    .font_weight(FontWeight::BOLD)
                    .child(div().flex_none().w(px(MERCHANT_WIDTH)).child("Merchant"))
                    .children(view.columns.iter().enumerate().map(|(index, column)| cell(column.clone().into(), view.today == Some(index))));
                let rows = view.rows.iter().map(|row| {
                    div()
                        .flex()
                        .items_center()
                        .py_0p5()
                        .border_t_1()
                        .border_color(rgb(t.overlay))
                        .child(
                            div()
                                .flex_none()
                                .w(px(MERCHANT_WIDTH))
                                .flex()
                                .flex_col()
                                .child(div().text_color(rgb(t.text)).child(row.name.clone()))
                                .children(row.note.clone().map(|note| div().text_size(px(10.)).text_color(rgb(t.muted)).child(note))),
                        )
                        .children(row.cells.iter().enumerate().map(|(index, value)| {
                            let today = view.today == Some(index);
                            let text = value.clone().unwrap_or_else(|| "--".to_owned());
                            cell(text.into(), today).text_color(rgb(if value.is_some() { if today { t.heading } else { t.text } } else { t.muted }))
                        }))
                });
                div()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap_2()
                            .px_4()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(t.overlay))
                            .child(div().flex_1().min_w_0().flex().flex_col().child(div().text_sm().text_color(rgb(t.text)).child(view.facts.clone())).child(div().text_xs().text_color(rgb(t.muted)).child(view.saved.clone())))
                            .child(refresh),
                    )
                    .child(
                        div()
                            .id("item-odds-table")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .p_4()
                            .overflow_y_scroll()
                            .text_xs()
                            .child(head)
                            .children(rows)
                            .when(view.rows.is_empty(), |table| table.child(div().pt_2().text_color(rgb(t.muted)).child("Bazaar DB has no merchant data for this item.")))
                            .child(div().pt_3().text_size(px(10.)).text_color(rgb(t.muted)).child("The chance the item is in stock when you visit that merchant that day and use its one reroll, from Bazaar DB's tracked runs.")),
                    )
            }
        };
        div().absolute().inset_0().flex().items_center().justify_center().child(panel(t, "item-odds").w(px(680.)).max_w(relative(0.95)).max_h(relative(0.9)).child(header).child(body))
    }

    /// At an encounter choice, the chance each owned item with saved odds is in stock at each
    /// merchant on offer today (and at any shop). Nothing when none have saved odds.
    pub(in crate::ui) fn render_choice_odds(&self, view: &LiveView) -> Option<Div> {
        let t = self.theme;
        let hero = view.hero.as_deref()?;
        let day = u32::try_from(view.day?).ok()?;
        let lines = item_odds::choice_lines(&self.item_odds, hero, &view.owned, &view.merchants, day);
        if lines.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(widgets::section_header([widgets::section_title(t, "ITEM ODDS"), widgets::badge(format!("Day {day}"), t.subtle, widgets::Tone::Soft, t)], []))
                .children(lines.into_iter().map(|line| {
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .text_xs()
                        .child(div().flex_none().w(px(MERCHANT_WIDTH)).truncate().text_color(rgb(t.text)).child(line.merchant))
                        .child(div().flex().flex_wrap().gap_x_3().children(line.items.into_iter().map(|(name, chance)| {
                            div().flex().gap_1().child(div().text_color(rgb(t.subtle)).child(name)).child(div().font_weight(FontWeight::BOLD).text_color(rgb(t.poison)).child(chance))
                        })))
                })),
        )
    }
}
