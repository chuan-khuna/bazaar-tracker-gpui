//! Item odds: the button on an item's tile (shown while it is hovered) that loads the item's odds
//! from Bazaar DB or, once they are saved, opens them; the pop-up with every merchant and day; and
//! at an encounter choice, the chances beside each merchant on offer.
//!
//! Loading goes through the web view, hidden: its shop odds page asks Bazaar DB for the item (the
//! page script finds it in the page's search the first time), and the Bazaar DB panel opens when that
//! does not work. Only a reply for the item asked about is saved.

use gpui::{Context, Div, FontWeight, SharedString, div, prelude::*, px, relative, rgb, rgba};

use crate::overlays::Overlay;
use crate::shell::settings::OddsTab;
use crate::overlays::item_odds::ItemOddsOverlay;
use crate::overlays::tracked_items::TrackedItemsOverlay;
use crate::tracking::item_odds::{self, Wanted};
use crate::tracking::live::LiveView;
use crate::tracking::run::board::Tile;
use crate::ui::TrackerApp;
use bazaar_tracker::history::History;
use crate::ui::widgets::{self, IconName, icon_button, panel, query_field};

/// Whose odds an item's tile asks for: the hero playing, the day whose column is marked, and
/// whether the items are the player's own (asked about at the tier they own) or not (the
/// opponent's, the shop's: asked about as not owned).
#[derive(Clone, Debug)]
pub(in crate::ui) struct OddsContext {
    pub hero: String,
    pub day: Option<u32>,
    pub owned: bool,
}

impl OddsContext {
    /// For `hero` on `day`, when the hero is known.
    pub fn new(hero: Option<&str>, day: Option<i32>) -> Option<Self> {
        let hero = hero.filter(|hero| !hero.is_empty())?;
        Some(Self { hero: hero.to_owned(), day: day.and_then(|day| u32::try_from(day).ok()), owned: true })
    }

    /// The same, for items the player does not own.
    pub fn not_owned(self) -> Self {
        Self { owned: false, ..self }
    }
}

/// The group a tile's hover shows its odds button in.
pub(in crate::ui) const TILE_GROUP: &str = "tile";

/// How long odds loading in the background may take before the Bazaar DB panel is shown.
#[cfg(windows)]
const BACKGROUND_PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// The most catalog items the Tracked items pop-up lists for what is typed.
const CANDIDATE_LIMIT: usize = 40;

/// How many item badges fill a row of cards: it sets a badge's width at an encounter choice.
const ODDS_COLUMNS: usize = 5;

/// How many merchants the Best shops section names for each item.
const BEST_SHOPS: usize = 3;

/// The width of the merchant column of the pop-up's table.
const MERCHANT_WIDTH: f32 = 150.;

impl TrackerApp {
    /// The tile's odds button, at its top right, seen while the tile is hovered (and always while its
    /// odds are being loaded). A card that is not an item, or has no card id, has none.
    pub(in crate::ui) fn odds_button(&self, id: SharedString, tile: &Tile, odds: &OddsContext, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let t = self.theme;
        if !tile.is_item {
            return None;
        }
        let tier = if odds.owned { tile.tier_name.clone() } else { item_odds::NOT_OWNED.to_owned() };
        let wanted = Wanted { template_id: tile.template_id.clone()?, name: tile.name.clone(), hero: odds.hero.clone(), tier };
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

    /// Load `wanted`'s odds, for this one click, in the web view without showing it: the shop odds
    /// page opens on the item if its Bazaar DB id is known, else the page script types its name into
    /// the page's search and picks it. The page then asks Bazaar DB for the odds itself. If that does
    /// not work (no match, Cloudflare's check, a changed page) or takes too long, the Bazaar DB panel
    /// opens so the player can finish it.
    pub(in crate::ui) fn ask_item_odds(&mut self, wanted: Wanted, cx: &mut Context<Self>) {
        // The player's own ask comes first: a load of many items stops for it.
        if self.odds_load.is_some() {
            self.stop_odds_load(cx);
        }
        match self.begin_item_odds_ask(wanted.clone(), cx) {
            Ok(()) => self.show_notice(format!("Loading the odds for {}…", wanted.name), cx),
            Err(why) => self.show_error(format!("Could not load the odds for {}: {why}", wanted.name), cx),
        }
    }

    /// Ask the hidden web view for `wanted`'s odds. If they have not come in time, the ask fails:
    /// see [`Self::item_odds_failed`].
    #[cfg(windows)]
    pub(in crate::ui) fn begin_item_odds_ask(&mut self, wanted: Wanted, cx: &mut Context<Self>) -> Result<(), String> {
        use bazaardb::webview::item_odds::{page_for, page_to_find};
        let page = match self.item_odds.url_id(&wanted.template_id) {
            Some(url_id) => page_for(Some(url_id), &wanted.hero, &wanted.tier),
            None => page_to_find(&wanted.name, &wanted.hero, &wanted.tier),
        };
        tracing::info!("item odds: asking Bazaar DB for {} ({}, {})", wanted.name, wanted.hero, wanted.tier);
        self.item_odds_pending = Some(wanted);
        self.item_odds_asks += 1;
        let ask = self.item_odds_asks;
        if let Err(why) = self.load_in_web_background(&page, cx) {
            self.item_odds_pending = None;
            return Err(why);
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BACKGROUND_PATIENCE).await;
            this.update(cx, |this, cx| {
                if this.item_odds_asks == ask {
                    this.item_odds_failed(None, "Bazaar DB took too long", cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
        Ok(())
    }

    #[cfg(not(windows))]
    pub(in crate::ui) fn begin_item_odds_ask(&mut self, _wanted: Wanted, _cx: &mut Context<Self>) -> Result<(), String> {
        Err("item odds load through the Bazaar DB panel, which is only on Windows".to_owned())
    }

    /// Loading the awaited item's odds in the background did not work, for `why`. While many items
    /// load, the item is skipped. Otherwise the Bazaar DB panel opens on the page so the player can
    /// pick the item or answer Cloudflare's check. `found` is the name a failed search was for; a
    /// report about another item is ignored.
    #[cfg(windows)]
    pub(in crate::ui) fn item_odds_failed(&mut self, found: Option<&str>, why: &str, cx: &mut Context<Self>) {
        let Some(wanted) = &self.item_odds_pending else { return };
        if found.is_some_and(|name| name != wanted.name) {
            return;
        }
        if self.odds_load.is_some() {
            self.skip_odds_load_item(why, cx);
            return;
        }
        if self.web.open {
            return;
        }
        let name = wanted.name.clone();
        tracing::info!("item odds: {name} did not load on its own; showing the Bazaar DB panel");
        // The web view steps aside while a pop-up is open, so close it.
        if self.overlay.is_some() {
            self.dismiss(cx);
        }
        self.reveal_web(cx);
        self.show_notice(format!("Bazaar DB needs you: pick {name} on its page"), cx);
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
        // A saved reply ends the ask: its timer must not open the panel.
        self.item_odds_asks += 1;
        if self.odds_load.is_some() {
            if let Some(note) = accepted.note {
                tracing::info!("item odds: {note}");
            }
            self.odds_load_item_saved(cx);
            return true;
        }
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

    /// The hero and day the Live island's odds are for: the game's, else the live Run's hero.
    fn odds_for(&self, view: &LiveView) -> Option<(String, u32)> {
        let run_hero = self.session.live().and_then(|run| self.session.run(run)).and_then(|run| run.hero.clone());
        Some((view.hero.clone().or(run_hero)?, u32::try_from(view.day?).ok()?))
    }

    /// The width of an odds badge: as wide as when ODDS_COLUMNS badges filled a row of cards, less
    /// the 4 px gaps.
    fn odds_badge_width(&self) -> f32 {
        (self.max_row_width() - 4. * (ODDS_COLUMNS - 1) as f32) / ODDS_COLUMNS as f32
    }

    /// A small badge drawn like the day buttons: a name, cut short with an ellipsis, a tier mark,
    /// the chance, then, given `hide` (a key and the item's card id and name), ✕, which hides the
    /// item in this Run.
    fn odds_badge(&self, name: &str, tier_mark: Option<&str>, chance: &str, hide: Option<(String, String, String)>, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        div()
            .flex()
            .flex_none()
            .w(px(self.odds_badge_width()))
            .min_w_0()
            .items_center()
            .gap_1()
            .h(px(widgets::BADGE_HEIGHT + 2.))
            .pl(px(6.))
            .pr(px(if hide.is_some() { 2. } else { 6. }))
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(t.highlight_low))
            .text_size(px(11.))
            .child(div().flex_1().min_w_0().truncate().text_color(rgb(t.subtle)).child(name.to_owned()))
            .children(tier_mark.map(|mark| div().flex_none().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child(mark.to_owned())))
            .child(div().flex_none().font_weight(FontWeight::BOLD).text_color(rgb(t.poison)).child(chance.to_owned()))
            .children(hide.map(|(key, id, name)| {
                div()
                    .id(SharedString::from(key))
                    .flex_none()
                    .px(px(2.))
                    .rounded_sm()
                    .cursor_pointer()
                    .text_size(px(10.))
                    .text_color(rgb(t.muted))
                    .hover(|style| style.text_color(rgb(t.loss)))
                    .tooltip(widgets::hint(t, vec![format!("Hide {name} in this run")]))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_hidden(&id, true, cx)))
                    .child("✕")
            }))
    }

    /// A named group of badges: the name in bold, then the badges, wrapping to as many a row as the
    /// panel holds; being of one width, a short last row lines up with the rows above.
    fn odds_group(&self, name: String, badges: Vec<Div>) -> Div {
        let t = self.theme;
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .text_xs()
            .child(div().font_weight(FontWeight::BOLD).text_color(rgb(t.text)).child(name))
            .child(div().flex().flex_wrap().gap_1().children(badges))
    }

    /// At an encounter choice, each merchant on offer that Bazaar DB knows, with a badge for the
    /// chance of each watched item (owned, or tracked in this Run) being in stock there today; "Any
    /// shop" comes last when the setting asks for it. Nothing when none of them have saved odds.
    pub(in crate::ui) fn choice_odds_groups(&self, view: &LiveView, cx: &mut Context<Self>) -> Option<Vec<Div>> {
        let (hero, day) = self.odds_for(view)?;
        let watched = item_odds::watched(&view.owned, &self.tracked_named(), &self.hidden());
        let groups = item_odds::choice_groups(&self.item_odds, &hero, &watched, &view.merchants, day, self.settings.get().show_any_shop_odds);
        if groups.is_empty() {
            return None;
        }
        Some(
            groups
                .iter()
                .enumerate()
                .map(|(row, group)| {
                    let badges = group
                        .items
                        .iter()
                        .enumerate()
                        .map(|(column, chance)| {
                            let hide = (format!("hide-odds-{row}-{column}"), chance.template_id.clone(), chance.name.clone());
                            self.odds_badge(&chance.name, chance.tier_mark.as_deref(), &chance.chance, Some(hide), cx)
                        })
                        .collect();
                    self.odds_group(group.merchant.clone(), badges)
                })
                .collect(),
        )
    }

    /// For each watched item (owned, or tracked in this Run), a badge for each of the merchants most
    /// likely to have it in stock today, out of every merchant Bazaar DB measured, best first.
    /// Nothing when none have saved odds.
    pub(in crate::ui) fn best_shops_groups(&self, view: &LiveView, cx: &mut Context<Self>) -> Option<Vec<Div>> {
        let (hero, day) = self.odds_for(view)?;
        let watched = item_odds::watched(&view.owned, &self.tracked_named(), &self.hidden());
        let best = item_odds::best_shops(&self.item_odds, &hero, &watched, day, BEST_SHOPS);
        if best.is_empty() {
            return None;
        }
        Some(
            best.iter()
                .map(|item| {
                    let name = match &item.tier_mark {
                        Some(mark) => format!("{} ({mark})", item.name),
                        None => item.name.clone(),
                    };
                    let badges = item.shops.iter().map(|(merchant, chance)| self.odds_badge(merchant, None, chance, None, cx)).collect();
                    self.odds_group(name, badges)
                })
                .collect(),
        )
    }

    /// The Live island's odds: a heading that folds them, with a tab each for ITEM ODDS (at an
    /// encounter choice) and BEST SHOPS, then the chosen tab's groups. Nothing outside a Run.
    pub(in crate::ui) fn render_odds_tabs(&self, view: &LiveView, cx: &mut Context<Self>) -> Option<Div> {
        let t = self.theme;
        let (_, day) = self.odds_for(view)?;
        let window = &self.settings.get().window;
        let (open, tab) = (window.odds_open, window.odds_tab);
        let groups = match tab {
            OddsTab::ItemOdds => self.choice_odds_groups(view, cx),
            OddsTab::BestShops => self.best_shops_groups(view, cx),
        };
        let empty = match tab {
            OddsTab::ItemOdds => "Shown at an encounter choice, for items with saved odds.",
            OddsTab::BestShops => "Shown for items with saved odds.",
        };
        let tab_button = |id: &'static str, label: &'static str, this_tab: OddsTab| {
            let chosen = tab == this_tab;
            div()
                .id(id)
                .pb_0p5()
                .border_b_2()
                .border_color(rgb(if chosen { t.accent } else { t.background }))
                .cursor_pointer()
                .child(widgets::section_title(t, label).text_color(rgb(if chosen { t.text } else { t.muted })))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.change_settings(
                        |settings| {
                            settings.window.odds_tab = this_tab;
                            settings.window.odds_open = true;
                        },
                        cx,
                    );
                    cx.notify();
                }))
        };
        let item_tab = tab_button("odds-tab-item-odds", "ITEM ODDS", OddsTab::ItemOdds);
        let best_tab = tab_button("odds-tab-best-shops", "BEST SHOPS", OddsTab::BestShops);
        let fold = div()
            .id("odds-fold")
            .cursor_pointer()
            .text_xs()
            .text_color(rgb(t.muted))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_odds(cx)))
            .child(if open { "▾" } else { "▸" });
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(fold)
                        .child(item_tab)
                        .child(best_tab)
                        .child(widgets::badge(format!("Day {day}"), t.subtle, widgets::Tone::Soft, t)),
                )
                .when(open, |section| match groups {
                    Some(groups) => section.children(groups),
                    None => section.child(div().text_xs().text_color(rgb(t.muted)).child(empty)),
                }),
        )
    }

    /// The Run items are tracked in: the one being played, else the one selected.
    pub(in crate::ui) fn tracking_run(&self) -> Option<String> {
        self.session.live().or(self.session.selected()).map(str::to_owned)
    }

    /// The hero of the Run items are tracked in, for their odds: the game's, while it is played.
    fn tracking_hero(&self) -> Option<String> {
        let run = self.tracking_run()?;
        let live_hero = if self.session.is_live(&run) { self.live.hero().map(str::to_owned) } else { None };
        live_hero.or_else(|| self.session.run(&run).and_then(|run| run.hero.clone()))
    }

    /// The card ids tracked in the tracking Run, read from the history once for each Run.
    pub(in crate::ui) fn tracked(&self) -> Vec<String> {
        self.run_items(&self.tracked_cache, "tracked", History::tracked_items)
    }

    /// The card ids hidden from the odds in the tracking Run, read once for each Run.
    fn hidden(&self) -> Vec<String> {
        self.run_items(&self.hidden_cache, "hidden", History::hidden_items)
    }

    /// The tracking Run's card ids that `read` gets from the history, kept in `cache` until the Run changes.
    fn run_items(&self, cache: &std::cell::RefCell<Option<(String, Vec<String>)>>, what: &str, read: fn(&History, &str) -> bazaar_tracker::history::Result<Vec<String>>) -> Vec<String> {
        let (Some(run), Some(history)) = (self.tracking_run(), self.history.as_deref()) else { return Vec::new() };
        let mut cache = cache.borrow_mut();
        if let Some((cached_run, ids)) = cache.as_ref()
            && *cached_run == run
        {
            return ids.clone();
        }
        let ids = read(history, &run).unwrap_or_else(|error| {
            tracing::error!("could not read the {what} items: {error}");
            Vec::new()
        });
        *cache = Some((run, ids.clone()));
        ids
    }

    /// The tracked items with their names.
    fn tracked_named(&self) -> Vec<(String, String)> {
        self.tracked().into_iter().map(|id| (id.clone(), self.names.card(Some(&id), &id))).collect()
    }

    /// The rows of the Tracked items pop-up for what is typed in it.
    pub(in crate::ui) fn tracked_candidates(&self, query: &str) -> Vec<item_odds::Candidate> {
        item_odds::candidates(&self.names.items(), &self.tracked(), &self.hidden(), query, CANDIDATE_LIMIT)
    }

    /// A row of the Tracked items pop-up picked: a hidden item is shown again, any other is
    /// tracked or untracked.
    pub(in crate::ui) fn pick_tracked_row(&mut self, template_id: String, name: String, hidden: bool, cx: &mut Context<Self>) {
        if hidden {
            self.set_hidden(&template_id, false, cx);
        } else {
            self.toggle_tracked(template_id, name, cx);
        }
    }

    /// Hide the item from the odds beside the merchants in the tracking Run, or show it again.
    pub(in crate::ui) fn set_hidden(&mut self, template_id: &str, hide: bool, cx: &mut Context<Self>) {
        let (Some(run), Some(history)) = (self.tracking_run(), self.history.clone()) else { return };
        let result = if hide { history.hide_item(&run, template_id, super::now_ms()) } else { history.unhide_item(&run, template_id) };
        self.hidden_cache.borrow_mut().take();
        if let Err(error) = result {
            self.show_error(format!("Could not change the hidden items: {error}"), cx);
            return;
        }
        if hide {
            self.show_notice(format!("Hid {} · show it again from Tracked items", self.names.card(Some(template_id), template_id)), cx);
        }
        cx.notify();
    }

    /// Track the item in the tracking Run, or stop tracking it. A newly tracked item with no odds
    /// saved for the Run's hero has them loaded.
    pub(in crate::ui) fn toggle_tracked(&mut self, template_id: String, name: String, cx: &mut Context<Self>) {
        let (Some(run), Some(history)) = (self.tracking_run(), self.history.clone()) else {
            self.show_notice("Pick a run to track items in", cx);
            return;
        };
        let tracked = self.tracked().iter().any(|id| id.eq_ignore_ascii_case(&template_id));
        let result = if tracked { history.untrack_item(&run, &template_id) } else { history.track_item(&run, &template_id, super::now_ms()) };
        self.tracked_cache.borrow_mut().take();
        if let Err(error) = result {
            self.show_error(format!("Could not change the tracked items: {error}"), cx);
            return;
        }
        if !tracked
            && let Some(hero) = self.tracking_hero()
            && self.item_odds.get(&template_id, &hero, item_odds::NOT_OWNED).is_none()
        {
            self.ask_item_odds(Wanted { template_id, name, hero, tier: item_odds::NOT_OWNED.to_owned() }, cx);
        }
        cx.notify();
    }

    /// The Tracked items pop-up: a search field, the items tracked in the Run, then the catalog's
    /// items that match what is typed. A click (or Enter) tracks or untracks one; a tracked item has
    /// a button for its odds, which loads them if none are saved.
    pub(in crate::ui) fn render_tracked_items_overlay(&self, overlay: &TrackedItemsOverlay, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let rows = self.tracked_candidates(&overlay.query);
        let hero = self.tracking_hero();
        let subtitle = match (self.tracking_run(), &hero) {
            (None, _) => "No run to track items in.".to_owned(),
            (Some(_), Some(hero)) => format!("Tracked in this run, as {hero}. Odds are for an item not owned."),
            (Some(_), None) => "Tracked in this run. Its hero is not known yet, so odds can not load.".to_owned(),
        };
        let mut list = div().id("tracked-items-list").flex().flex_col().p_1().max_h(px(360.)).overflow_y_scroll();
        for (index, row) in rows.iter().enumerate() {
            let group = |row: &item_odds::Candidate| if row.hidden { "HIDDEN" } else if row.tracked { "TRACKED" } else { "ITEMS" };
            if index == 0 || group(&rows[index - 1]) != group(row) {
                list = list.child(div().px_2().pt_1p5().pb_0p5().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child(group(row)));
            }
            let (id, name) = (row.template_id.clone(), row.name.clone());
            let odds = hero.as_ref().filter(|_| row.tracked && !row.hidden).map(|hero| {
                let wanted = Wanted { template_id: id.clone(), name: name.clone(), hero: hero.clone(), tier: item_odds::NOT_OWNED.to_owned() };
                let saved = self.item_odds.get(&id, hero, item_odds::NOT_OWNED).is_some();
                let pending = self.item_odds_pending.as_ref() == Some(&wanted);
                let (icon, colour) = match (saved, pending) {
                    (_, true) => (IconName::Loader, t.subtle),
                    (true, false) => (IconName::ChartPie, t.accent),
                    (false, false) => (IconName::Search, t.subtle),
                };
                icon_button(t, SharedString::from(format!("tracked-odds-{index}")), widgets::icon(icon, colour)).on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.dismiss(cx);
                    this.item_odds_clicked(wanted.clone(), None, cx);
                }))
            });
            let mark = if row.hidden { ("Show", t.success) } else if row.tracked { ("✕", t.loss) } else { ("+ Track", t.success) };
            let hidden = row.hidden;
            list = list.child(
                div()
                    .id(SharedString::from(format!("tracked-{index}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .when(index == overlay.selected, |line| line.bg(rgb(t.overlay)))
                    .hover(|style| style.bg(rgb(t.overlay)))
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_tracked_row(id.clone(), name.clone(), hidden, cx)))
                    .child(div().flex_1().min_w_0().truncate().text_color(rgb(t.text)).child(row.name.clone()))
                    .children(odds)
                    .child(div().flex_none().text_xs().text_color(rgb(mark.1)).child(mark.0)),
            );
        }
        if rows.is_empty() {
            let text = if self.names.items().is_empty() {
                "The card catalog has not loaded yet."
            } else if overlay.query.is_empty() {
                "Type to find an item to track."
            } else {
                "No items match."
            };
            list = list.child(div().px_2().py_1().text_sm().text_color(rgb(t.muted)).child(text));
        }
        div().absolute().top(px(56.)).left_0().right_0().flex().justify_center().child(
            panel(t, "tracked-items")
                .w(px(420.))
                .child(query_field(t, &overlay.query, "Find an item to track…"))
                .child(div().px_3().pt_2().text_xs().text_color(rgb(t.muted)).child(subtitle))
                .child(list),
        )
    }
}
