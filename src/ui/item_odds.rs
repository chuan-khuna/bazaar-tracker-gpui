//! Item odds: the button on an item's tile (shown while it is hovered) that loads the item's odds
//! from Bazaar DB or, once they are saved, opens them; the pop-up with every merchant and day; and
//! at an encounter choice, the chances beside each merchant on offer.
//!
//! Loading goes through the web view, hidden: its shop odds page asks Bazaar DB for the item (the
//! page script finds it in the page's search the first time), and the Bazaar DB panel opens when that
//! does not work. Only a reply for the item asked about is saved.

use gpui::{Context, Div, FontWeight, SharedString, div, prelude::*, px, relative, rgb, rgba};

use crate::overlays::Overlay;
use crate::shell::settings::{OddsFilter, OddsTab};
use crate::overlays::item_odds::ItemOddsOverlay;
use crate::overlays::tracked_items::TrackedItemsOverlay;
use crate::tracking::item_odds::{self, OddsGrid, Wanted};
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

/// How many merchants the Best shops section names for each item.
const BEST_SHOPS: usize = 3;

/// The odds tables: the height of a row, and of one whose cells name their merchant over the
/// chance, the share of the table's width the item names take and the most they take, and how
/// many rows the Live island shows before the rest scroll.
const ODDS_ROW_HEIGHT: f32 = 22.;
const ODDS_TWO_LINE_HEIGHT: f32 = 34.;
const ODDS_NAME_SHARE: f32 = 0.32;
const ODDS_NAME_WIDTH: f32 = 140.;
const ODDS_VISIBLE_ROWS: usize = 5;

/// The longest the odds search can get.
const ODDS_QUERY_MAX: usize = 40;

/// The width of the merchant column of the pop-up's table.
const MERCHANT_WIDTH: f32 = 150.;

/// `for the item not owned`, or `for the item owned Silver`.
fn tier_phrase(tier: &str) -> String {
    if tier == item_odds::NOT_OWNED { "for the item not owned".to_owned() } else { format!("for the item owned {tier}") }
}

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
    #[cfg(windows)]
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
        // What was saved, which may be another hero or tier than asked, if the player changed them.
        let asked = Wanted { hero: accepted.saved.hero.clone(), tier: accepted.saved.tier.clone(), ..wanted.clone() };
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
            None => self.show_success(format!("Saved the odds for {name}"), cx),
        }
        // The reply named the tiers the item comes in: load those not saved yet, in the background.
        self.load_odds_of(item_odds::missing_tiers(&self.item_odds, &asked), cx);
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
        let tiers = div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(rgb(t.overlay))
            .child(div().pr_1().text_xs().text_color(rgb(t.muted)).child("Tier owned"))
            .children(item_odds::tier_choices(&self.item_odds, item).into_iter().map(|choice| {
                let chosen = choice.tier == item.tier;
                let hint = if choice.saved { format!("Show the odds {}", tier_phrase(&choice.tier)) } else { format!("Load the odds {} from Bazaar DB", tier_phrase(&choice.tier)) };
                let tier = choice.tier.clone();
                self.filter_button(format!("item-odds-tier-{}", choice.tier), choice.label, chosen)
                    .when(!choice.saved && !chosen, |button| button.opacity(0.6))
                    .tooltip(widgets::hint(t, vec![hint]))
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_odds_tier(tier.clone(), cx)))
            }));
        let loading = self.item_odds_pending.as_ref() == Some(item);
        let body = match entry {
            None if loading => div().p_4().text_sm().text_color(rgb(t.muted)).child(format!("Loading the odds {} from Bazaar DB…", tier_phrase(&item.tier))),
            None => div().p_4().text_sm().text_color(rgb(t.muted)).child(format!("No odds saved {}.", tier_phrase(&item.tier))),
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
        div().absolute().inset_0().flex().items_center().justify_center().child(panel(t, "item-odds").w(px(680.)).max_w(relative(0.95)).max_h(relative(0.9)).child(header).child(tiers).child(body))
    }

    /// Show the item odds pop-up's item at `tier` owned, loading its odds from Bazaar DB when none are
    /// saved. The pop-up stays open, and shows them once they come.
    fn pick_odds_tier(&mut self, tier: String, cx: &mut Context<Self>) {
        let Some(Overlay::ItemOdds(overlay)) = &mut self.overlay else { return };
        if overlay.item.tier == tier {
            return;
        }
        overlay.item.tier = tier;
        let wanted = overlay.item.clone();
        let missing = self.item_odds.get(&wanted.template_id, &wanted.hero, &wanted.tier).is_none();
        if missing && self.item_odds_pending.as_ref() != Some(&wanted) {
            self.ask_item_odds(wanted, cx);
        }
        cx.notify();
    }

    /// The hero and day the Live island's odds are for: the game's, else the live Run's hero.
    pub(in crate::ui) fn odds_for(&self, view: &LiveView) -> Option<(String, u32)> {
        let run_hero = self.session.live().and_then(|run| self.session.run(run)).and_then(|run| run.hero.clone());
        Some((view.hero.clone().or(run_hero)?, u32::try_from(view.day?).ok()?))
    }

    /// A table of odds, drawn like the Events table: a header naming the columns, then a row for each
    /// item, its name (with ✕ to hide it in this Run, given `hide`) and a cell for each column, `—`
    /// where the merchant does not carry it. With `visible_rows`, only that many rows show and the
    /// rest scroll under the header. No wider than a row of cards. Given open (the hero and the day),
    /// a click on an item's name opens its odds.
    pub(in crate::ui) fn odds_table(&self, id: &'static str, grid: &OddsGrid, hide: bool, visible_rows: Option<usize>, open: Option<(String, u32)>, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let line = || div().flex().items_center().gap_2().h(px(ODDS_ROW_HEIGHT)).px_2();
        // A cell that names its merchant (Best shops) puts the name over the chance, so a narrow
        // column cuts neither short.
        let two_lines = grid.rows.iter().flat_map(|row| row.cells.iter().flatten()).any(|cell| cell.merchant.is_some());
        let row_height = if two_lines { ODDS_TWO_LINE_HEIGHT } else { ODDS_ROW_HEIGHT };
        let header = line()
            .bg(rgb(t.highlight_low))
            .border_b_1()
            .border_color(rgb(t.overlay))
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(t.muted))
            .child(div().flex_none().w(relative(ODDS_NAME_SHARE)).max_w(px(ODDS_NAME_WIDTH)).child("ITEM"))
            .children(grid.columns.iter().map(|column| div().flex_1().min_w_0().truncate().text_right().child(column.clone())));
        let rows: Vec<Div> = grid
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let name = match &row.tier_mark {
                    Some(mark) => format!("{} ({mark})", row.name),
                    None => row.name.clone(),
                };
                let (template_id, item_name) = (row.template_id.clone(), row.name.clone());
                let label = div().id(SharedString::from(format!("{id}-name-{index}"))).min_w_0().truncate().text_color(rgb(t.text)).child(name);
                let label = match &open {
                    Some((hero, day)) => {
                        let wanted = Wanted { template_id: row.template_id.clone(), name: row.name.clone(), hero: hero.clone(), tier: row.tier.clone() };
                        let day = Some(*day);
                        label
                            .cursor_pointer()
                            .hover(|style| style.text_color(rgb(t.accent)))
                            .tooltip(widgets::hint(t, vec![format!("Show the odds of {item_name}")]))
                            .on_click(cx.listener(move |this, _, _, cx| this.item_odds_clicked(wanted.clone(), day, cx)))
                    }
                    None => label,
                };
                let hide_button = hide.then(|| {
                    div()
                        .id(SharedString::from(format!("{id}-hide-{index}")))
                        .flex_none()
                        .px(px(2.))
                        .rounded_sm()
                        .cursor_pointer()
                        .text_size(px(10.))
                        .text_color(rgb(t.muted))
                        .hover(|style| style.text_color(rgb(t.loss)))
                        .tooltip(widgets::hint(t, vec![format!("Hide {item_name} in this run")]))
                        .on_click(cx.listener(move |this, _, _, cx| this.set_hidden(&template_id, true, cx)))
                        .child("✕")
                });
                let cells = row.cells.iter().map(|cell| {
                    let cell_div = div().flex().flex_col().flex_1().min_w_0().items_end().justify_center();
                    match cell {
                        None => cell_div.text_color(rgb(t.highlight_med)).child("—"),
                        Some(cell) => cell_div
                            .children(cell.merchant.clone().map(|merchant| div().max_w_full().truncate().text_color(rgb(t.subtle)).child(merchant)))
                            .child(div().flex_none().whitespace_nowrap().font_weight(FontWeight::BOLD).text_color(rgb(t.poison)).child(cell.chance.clone())),
                    }
                });
                line()
                    .h(px(row_height))
                    .text_xs()
                    .when(index % 2 == 1, |line| line.bg(rgb(t.highlight_low)))
                    .child(div().flex().flex_none().w(relative(ODDS_NAME_SHARE)).max_w(px(ODDS_NAME_WIDTH)).items_center().gap_1().child(label).children(hide_button))
                    .children(cells)
            })
            .collect();
        let count = rows.len();
        // The rows past those in view, when the table scrolls.
        let more = visible_rows.map_or(0, |visible| count.saturating_sub(visible));
        let body = if rows.is_empty() {
            let columns = grid.columns.join(", ");
            div().child(line().text_xs().text_color(rgb(t.muted)).child(format!("None of your items at {columns}")))
        } else {
            match visible_rows {
                Some(visible) => div()
                    .relative()
                    .child(
                        div()
                            .id(SharedString::from(format!("{id}-scroll")))
                            .max_h(px(visible as f32 * row_height))
                            .overflow_y_scroll()
                            .track_scroll(&self.odds_scroll)
                            // gpui scrolls every scroll area under the pointer: while the table has
                            // rows to scroll to, the wheel scrolls it alone, not the island too. Its
                            // own scrolling runs before this.
                            .when(more > 0, |scroll| scroll.on_scroll_wheel(|_, _, cx| cx.stop_propagation()))
                            .children(rows),
                    )
                    .child(widgets::scrollbar(t, SharedString::from(format!("{id}-scrollbar")), &self.odds_scroll)),
                None => div().children(rows),
            }
        };
        // Says the table scrolls, and how far.
        let footer = (more > 0).then(|| {
            let rows_word = if more == 1 { "row" } else { "rows" };
            line()
                .h(px(ODDS_ROW_HEIGHT * 0.8))
                .justify_center()
                .border_t_1()
                .border_color(rgb(t.overlay))
                .text_size(px(10.))
                .text_color(rgb(t.muted))
                .child(format!("↕ {count} items · scroll the table for {more} more {rows_word}"))
        });
        div()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(self.max_row_width()))
            .rounded_md()
            .border_1()
            .border_color(rgb(t.overlay))
            .overflow_hidden()
            .child(header)
            .child(body)
            .children(footer)
    }

    /// Whether the Run selected is the one being played, so its Events are the live Run's.
    fn watching_live_run(&self) -> bool {
        self.session.selected().is_some_and(|id| self.session.is_live(id))
    }

    /// The watched items the odds filter picks (all, the board's, the stash's or the tracked), and,
    /// while there is a search, those of them it finds and the other items it finds for `hero`.
    fn filtered_watched(&self, view: &LiveView, hero: &str) -> Vec<item_odds::Owned> {
        let watched = item_odds::watched_by(self.settings.get().window.odds_filter, &view.board_owned, &view.stash_owned, &self.tracked_named(), &self.hidden());
        item_odds::search(watched, &self.names.catalog_items(), hero, &self.odds_query)
    }

    /// A key to the odds search: typing adds to it, Backspace takes a letter off, Enter is done,
    /// and Escape clears it (or, when it is clear, is done).
    pub(in crate::ui) fn odds_search_key(&mut self, key: &str, typed: Option<&str>) {
        match key {
            "enter" => self.odds_search_focused = false,
            "escape" if self.odds_query.is_empty() => self.odds_search_focused = false,
            "escape" => self.odds_query.clear(),
            "backspace" => {
                self.odds_query.pop();
            }
            _ => {
                if let Some(typed) = typed.filter(|typed| !typed.chars().any(char::is_control))
                    && self.odds_query.chars().count() < ODDS_QUERY_MAX
                {
                    self.odds_query.push_str(typed);
                }
            }
        }
        self.odds_scroll.scroll_to_top_of_item(0);
    }

    /// The search field beside the odds tabs: what is typed, with a caret while it takes the typing,
    /// or what it is for; a ✕ clears it.
    fn render_odds_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let (focused, query) = (self.odds_search_focused, self.odds_query.clone());
        let caret = || div().flex_none().w(px(1.)).h(px(12.)).bg(rgb(t.text));
        div()
            .id("odds-search")
            .flex()
            .flex_1()
            .min_w(px(90.))
            .max_w(px(220.))
            .items_center()
            .gap_1()
            .h(px(22.))
            .px_1p5()
            .rounded_md()
            .border_1()
            .border_color(rgb(if focused { t.accent } else { t.overlay }))
            .cursor_text()
            .text_xs()
            .on_click(cx.listener(|this, _, _, cx| {
                this.odds_search_focused = true;
                cx.notify();
            }))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.odds_search_focused {
                    this.odds_search_focused = false;
                    cx.notify();
                }
            }))
            .child(widgets::icon(widgets::IconName::Search, t.muted).size(px(11.)).flex_none())
            .child(
                div().flex().flex_1().min_w_0().items_center().overflow_hidden().map(|text| match (query.is_empty(), focused) {
                    (true, true) => text.child(caret()).child(div().pl_1().truncate().text_color(rgb(t.muted)).child("Search items")),
                    (true, false) => text.child(div().truncate().text_color(rgb(t.muted)).child("Search items")),
                    (false, true) => text.child(div().truncate().text_color(rgb(t.text)).child(query.clone())).child(caret()),
                    (false, false) => text.child(div().truncate().text_color(rgb(t.text)).child(query.clone())),
                }),
            )
            .when(!query.is_empty(), |field| {
                field.child(
                    widgets::icon_button(t, "odds-search-clear", div().text_color(rgb(t.subtle)).child("✕")).on_click(cx.listener(|this, _, _, cx| {
                        this.odds_query.clear();
                        cx.stop_propagation();
                        cx.notify();
                    })),
                )
            })
    }

    /// While a search finds items with no saved odds, a line that says so and loads them.
    fn render_search_load(&self, view: &LiveView, cx: &mut Context<Self>) -> Option<Div> {
        let t = self.theme;
        if self.odds_query.trim().is_empty() {
            return None;
        }
        let (hero, _) = self.odds_for(view)?;
        let missing = item_odds::without_odds(&self.filtered_watched(view, &hero), &self.item_odds, &hero);
        if missing.is_empty() {
            return None;
        }
        let count = missing.len();
        let busy = self.odds_load.is_some();
        let label = if busy { "Loading…" } else { "Load their odds" };
        let overlay = t.overlay;
        let button = gpui_base::Button::new("odds-search-load")
            .focusable(false)
            .tab_stop(false)
            .flex_none()
            .px_2()
            .h(px(20.))
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(if busy { t.highlight_med } else { t.accent }))
            .text_xs()
            .text_color(rgb(if busy { t.muted } else { t.text }))
            .when(!busy, |button| button.cursor_pointer().hover(move |style| style.bg(rgb(overlay))).on_click(cx.listener(move |this, _, _, cx| this.load_odds_of(missing.clone(), cx))))
            .child(label);
        let items = if count == 1 { "item" } else { "items" };
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(rgb(t.muted))
                .child(format!("{count} {items} found have no saved odds yet."))
                .child(button),
        )
    }

    /// At an encounter choice, the merchants on offer (in a shop, the shop's merchant) and the
    /// chance of each watched item (owned, or tracked in this Run) being in stock there today; "Any
    /// shop" comes last when the setting asks for it. Nothing when none of them have saved odds.
    pub(in crate::ui) fn choice_odds_grid(&self, view: &LiveView) -> Option<OddsGrid> {
        let (hero, day) = self.odds_for(view)?;
        let watched = self.filtered_watched(view, &hero);
        // At a choice, the merchants on offer; in a shop, the merchant whose shop it is.
        let merchants = match (view.merchants.is_empty(), view.in_shop && self.watching_live_run()) {
            (false, _) => view.merchants.clone(),
            (true, true) => item_odds::current_merchant(self.session.events(), &self.names).into_iter().collect(),
            (true, false) => Vec::new(),
        };
        let groups = item_odds::choice_groups(&self.item_odds, &hero, &watched, &merchants, day, self.settings.get().show_any_shop_odds);
        (!groups.is_empty()).then(|| item_odds::choice_grid(&groups))
    }

    /// For each watched item (owned, or tracked in this Run), the merchants most likely to have it in
    /// stock today, out of every merchant Bazaar DB measured, best first. Nothing when none have
    /// saved odds.
    pub(in crate::ui) fn best_shops_grid(&self, view: &LiveView) -> Option<OddsGrid> {
        let (hero, day) = self.odds_for(view)?;
        let watched = self.filtered_watched(view, &hero);
        let best = item_odds::best_shops(&self.item_odds, &hero, &watched, day, BEST_SHOPS);
        (!best.is_empty()).then(|| item_odds::best_grid(&best, BEST_SHOPS))
    }

    /// The Live island's odds: a heading that folds them, with a tab each for ITEM ODDS (at an
    /// encounter choice) and BEST SHOPS, the day, a search and the filter, then the chosen tab's
    /// groups. Nothing outside a Run.
    pub(in crate::ui) fn render_odds_tabs(&self, view: &LiveView, cx: &mut Context<Self>) -> Option<Div> {
        let t = self.theme;
        let (hero, day) = self.odds_for(view)?;
        let odds_of = Some((hero, day));
        let window = &self.settings.get().window;
        let (open, tab) = (window.odds_open, window.odds_tab);
        let table = match tab {
            OddsTab::ItemOdds => self.choice_odds_grid(view).map(|grid| self.odds_table("item-odds", &grid, true, Some(ODDS_VISIBLE_ROWS), odds_of.clone(), cx)),
            OddsTab::BestShops => self.best_shops_grid(view).map(|grid| self.odds_table("best-shops", &grid, false, Some(ODDS_VISIBLE_ROWS), odds_of.clone(), cx)),
        };
        let query = self.odds_query.trim();
        let empty = match (tab, query.is_empty()) {
            (OddsTab::ItemOdds, true) => "Shown at an encounter choice or in a merchant's shop, for items with saved odds.".to_owned(),
            (OddsTab::BestShops, true) => "Shown for items with saved odds.".to_owned(),
            (OddsTab::ItemOdds, false) => format!("Nothing found for \"{query}\" with saved odds, or not at an encounter choice or in a shop."),
            (OddsTab::BestShops, false) => format!("Nothing found for \"{query}\" with saved odds."),
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
        // Which items both tables list, on the right of the tabs.
        let chosen = window.odds_filter;
        let filters = OddsFilter::ALL.map(|filter| {
            self.filter_button(format!("odds-filter-{}", filter.label()), filter.label().to_owned(), filter == chosen).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| settings.window.odds_filter = filter, cx);
                cx.notify();
            }))
        });
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
                        .child(widgets::badge(format!("Day {day}"), t.subtle, widgets::Tone::Soft, t))
                        .child(self.render_odds_search(cx))
                        .child(div().flex().flex_none().items_center().gap_1().ml_auto().children(filters)),
                )
                .when(open, |section| {
                    let section = match table {
                        Some(table) => section.child(table),
                        None => section.child(div().text_xs().text_color(rgb(t.muted)).child(empty)),
                    };
                    section.children(self.render_search_load(view, cx))
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

    /// The day of the Run items are tracked in, whose odds column is marked: the game's, while it is
    /// played.
    fn tracking_day(&self) -> Option<u32> {
        let run = self.tracking_run()?;
        let live_day = if self.session.is_live(&run) { self.live.day() } else { None };
        live_day.or_else(|| self.session.run(&run).and_then(|run| run.day)).and_then(|day| u32::try_from(day).ok())
    }

    /// The card ids tracked in the tracking Run: those pinned for its hero, then the Run's own.
    pub(in crate::ui) fn tracked(&self) -> Vec<String> {
        if self.tracking_run().is_none() {
            return Vec::new();
        }
        let mut ids = self.pinned();
        for id in self.run_items(&self.tracked_cache, "tracked", History::tracked_items) {
            if !ids.iter().any(|other| other.eq_ignore_ascii_case(&id)) {
                ids.push(id);
            }
        }
        ids
    }

    /// The card ids pinned for the tracking Run's hero, read from the history once for each hero.
    pub(in crate::ui) fn pinned(&self) -> Vec<String> {
        let (Some(hero), Some(history)) = (self.tracking_hero(), self.history.as_deref()) else { return Vec::new() };
        let mut cache = self.pinned_cache.borrow_mut();
        if let Some((cached_hero, ids)) = cache.as_ref()
            && *cached_hero == hero
        {
            return ids.clone();
        }
        let ids = history.pinned_items(&hero).unwrap_or_else(|error| {
            tracing::error!("could not read the pinned items: {error}");
            Vec::new()
        });
        *cache = Some((hero, ids.clone()));
        ids
    }

    /// The card ids hidden from the odds in the tracking Run, read once for each Run.
    pub(in crate::ui) fn hidden(&self) -> Vec<String> {
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
    pub(in crate::ui) fn tracked_named(&self) -> Vec<(String, String)> {
        self.tracked().into_iter().map(|id| (id.clone(), self.names.card(Some(&id), &id))).collect()
    }

    /// The rows of the Tracked items pop-up for what is typed in it.
    pub(in crate::ui) fn tracked_candidates(&self, query: &str) -> Vec<item_odds::Candidate> {
        item_odds::candidates(&self.names.items(), &self.tracked(), &self.hidden(), &self.pinned(), query, CANDIDATE_LIMIT)
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

    /// Track the item in the tracking Run, or stop tracking it (unpinning it too, if pinned). A newly
    /// tracked item with no odds saved for the Run's hero has them loaded.
    pub(in crate::ui) fn toggle_tracked(&mut self, template_id: String, name: String, cx: &mut Context<Self>) {
        let (Some(run), Some(history)) = (self.tracking_run(), self.history.clone()) else {
            self.show_notice("Pick a run to track items in", cx);
            return;
        };
        let tracked = self.tracked().iter().any(|id| id.eq_ignore_ascii_case(&template_id));
        let pinned = self.pinned().iter().any(|id| id.eq_ignore_ascii_case(&template_id));
        let mut result = if tracked { history.untrack_item(&run, &template_id) } else { history.track_item(&run, &template_id, super::now_ms()) };
        if tracked
            && pinned
            && let Some(hero) = self.tracking_hero()
        {
            result = result.and_then(|()| history.unpin_item(&hero, &template_id));
        }
        self.tracked_cache.borrow_mut().take();
        self.pinned_cache.borrow_mut().take();
        if let Err(error) = result {
            self.show_error(format!("Could not change the tracked items: {error}"), cx);
            return;
        }
        if !tracked {
            self.load_tracked_odds(template_id, name, cx);
        }
        cx.notify();
    }

    /// Pin the item for the tracking Run's hero, so it is tracked in each of the hero's Runs, or
    /// unpin it (it stays tracked in this Run). A newly pinned item has its odds loaded if none are saved.
    pub(in crate::ui) fn toggle_pinned(&mut self, template_id: String, name: String, cx: &mut Context<Self>) {
        let (Some(run), Some(hero), Some(history)) = (self.tracking_run(), self.tracking_hero(), self.history.clone()) else {
            self.show_notice("Items are pinned for a hero: pick a run whose hero is known", cx);
            return;
        };
        let pinned = self.pinned().iter().any(|id| id.eq_ignore_ascii_case(&template_id));
        let hero_name = bazaar_tracker::enums::hero_names::display_name(&hero).to_owned();
        // An unpinned item stays tracked in this Run, so the click does not make it vanish.
        let result = if pinned { history.track_item(&run, &template_id, super::now_ms()).and_then(|()| history.unpin_item(&hero, &template_id)) } else { history.pin_item(&hero, &template_id, super::now_ms()) };
        self.tracked_cache.borrow_mut().take();
        self.pinned_cache.borrow_mut().take();
        if let Err(error) = result {
            self.show_error(format!("Could not change the pinned items: {error}"), cx);
            return;
        }
        if pinned {
            self.show_notice(format!("Unpinned {name} for {hero_name}: still tracked in this run"), cx);
        } else {
            self.show_notice(format!("Pinned {name}: tracked in every {hero_name} run"), cx);
            self.load_tracked_odds(template_id, name, cx);
        }
        cx.notify();
    }

    /// Load a newly tracked or pinned item's odds, as not owned, if none are saved for the Run's hero.
    fn load_tracked_odds(&mut self, template_id: String, name: String, cx: &mut Context<Self>) {
        if let Some(hero) = self.tracking_hero()
            && self.item_odds.get(&template_id, &hero, item_odds::NOT_OWNED).is_none()
        {
            self.ask_item_odds(Wanted { template_id, name, hero, tier: item_odds::NOT_OWNED.to_owned() }, cx);
        }
    }

    /// The Tracked items pop-up: a search field, the items tracked in the Run, then the catalog's
    /// items that match what is typed. A click (or Enter) tracks or untracks one; a tracked item has
    /// a button for its odds, which loads them if none are saved.
    pub(in crate::ui) fn render_tracked_items_overlay(&self, overlay: &TrackedItemsOverlay, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let rows = self.tracked_candidates(&overlay.query);
        let hero = self.tracking_hero();
        let day = self.tracking_day();
        let subtitle = match (self.tracking_run(), &hero) {
            (None, _) => "No run to track items in.".to_owned(),
            (Some(_), Some(hero)) => format!("Tracked in this run, as {0}. Pin an item to track it in every {0} run. Odds are for an item not owned.", bazaar_tracker::enums::hero_names::display_name(hero)),
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
                    this.item_odds_clicked(wanted.clone(), day, cx);
                }))
            });
            let pin = hero.as_ref().filter(|_| row.tracked && !row.hidden).map(|hero| {
                let hero_name = bazaar_tracker::enums::hero_names::display_name(hero);
                let hint = if row.pinned { format!("Pinned: tracked in every {hero_name} run. Click to unpin") } else { format!("Pin: track in every {hero_name} run") };
                let (id, name) = (id.clone(), name.clone());
                icon_button(t, SharedString::from(format!("tracked-pin-{index}")), widgets::icon(IconName::Pin, if row.pinned { t.accent } else { t.muted }))
                    .when(!row.pinned, |button| button.opacity(0.6))
                    .tooltip(widgets::hint(t, vec![hint]))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_pinned(id.clone(), name.clone(), cx);
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
                    .children(pin)
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
