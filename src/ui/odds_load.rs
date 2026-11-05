//! Loading the odds of many items at once, from the Settings overlay: the player picks heroes (and
//! whether to take the neutral items too), and each item's odds are asked for in turn through the
//! hidden web view, as one item's are. An item that does not load is skipped; the status bar shows
//! which item is loading and how far the load has got. Between one item and the next the load
//! waits a moment, picked anew each time, so Bazaar DB is asked at a pace a person could keep.

use std::collections::{BTreeSet, VecDeque};
use std::hash::{BuildHasher, RandomState};
use std::time::Duration;

use gpui::{Context, Div, FontWeight, div, prelude::*, px, rgb};
use gpui_base::Button;

use crate::tracking::item_odds::{self, Wanted};
use crate::tracking::live::Live;
use crate::tracking::status::Connection;
use crate::ui::TrackerApp;
use crate::ui::widgets;

/// The shortest and longest wait between one item's ask and the next.
const GAP_MIN: Duration = Duration::from_millis(500);
const GAP_MAX: Duration = Duration::from_millis(4000);

/// A number picked at random (the standard library's hash keys are seeded from the system).
fn random() -> u64 {
    RandomState::new().hash_one(())
}

/// How long to wait before the next ask: anywhere from [`GAP_MIN`] to [`GAP_MAX`].
fn gap() -> Duration {
    let spread = u64::try_from((GAP_MAX - GAP_MIN).as_millis()).unwrap_or(0);
    GAP_MIN + Duration::from_millis(random() % (spread + 1))
}

/// What the Settings overlay's item odds section has picked. Kept while the app runs.
#[derive(Debug)]
pub(in crate::ui) struct OddsLoadPick {
    pub heroes: BTreeSet<String>,
    pub neutral: bool,
}

impl Default for OddsLoadPick {
    fn default() -> Self {
        Self { heroes: BTreeSet::new(), neutral: true }
    }
}

/// A load of many items' odds under way.
#[derive(Debug)]
pub(in crate::ui) struct OddsLoad {
    /// Tells this load apart from one started later, for a wait that ends after it stopped.
    id: u64,
    queue: VecDeque<Wanted>,
    total: usize,
    saved: usize,
    skipped: usize,
    /// The item being asked for now.
    current: Option<Wanted>,
}

impl OddsLoad {
    /// How far it has got, for the status bar: `Item odds 12/340 · Pasta Maker (Jules) · 3 skipped`.
    fn describe(&self) -> String {
        let done = self.saved + self.skipped;
        let mut text = format!("Item odds {}/{}", (done + 1).min(self.total), self.total);
        if let Some(current) = &self.current {
            text.push_str(&format!(" · {} ({})", current.name, bazaar_tracker::enums::hero_names::display_name(&current.hero)));
        }
        if self.skipped > 0 {
            text.push_str(&format!(" · {} skipped", self.skipped));
        }
        text
    }
}

impl TrackerApp {
    /// Start loading the odds of the picked heroes' items that have none saved.
    fn start_odds_load(&mut self, cx: &mut Context<Self>) {
        let heroes: Vec<String> = self.odds_load_pick.heroes.iter().cloned().collect();
        let queue = item_odds::load_queue(&self.names.catalog_items(), &heroes, self.odds_load_pick.neutral, &self.item_odds);
        if queue.is_empty() {
            self.show_notice("Every picked item already has its odds saved", cx);
            return;
        }
        tracing::info!("item odds: loading {} items for {}", queue.len(), heroes.join(", "));
        self.odds_load = Some(OddsLoad { id: random(), total: queue.len(), queue: queue.into(), saved: 0, skipped: 0, current: None });
        self.next_odds_load_item(cx);
    }

    /// Load in the background the odds the Live island's item odds need and has not got: each item
    /// on the board or in the stash at the tier owned (the chance to find it to upgrade), and each
    /// tracked item as not owned, less the hidden ones. Each is asked for once while the app runs;
    /// nothing starts while another load or ask is under way, and the next board read tries again.
    pub(in crate::ui) fn load_watched_odds(&mut self, cx: &mut Context<Self>) {
        if self.odds_load.is_some() || self.item_odds_pending.is_some() {
            return;
        }
        let attached = self.status.connection == Connection::Tracking;
        let Live::View(view) = self.live.view(attached, self.session.live().is_some(), &self.names) else { return };
        let Some((hero, _)) = self.odds_for(&view) else { return };
        let watched = item_odds::watched(&view.owned, &self.tracked_named(), &self.hidden());
        let queue: VecDeque<Wanted> = watched
            .into_iter()
            .filter(|item| self.item_odds.get(&item.template_id, &hero, &item.tier).is_none())
            .filter(|item| self.watched_odds_asked.insert((item.template_id.to_lowercase(), hero.clone(), item.tier.clone())))
            .map(|item| Wanted { template_id: item.template_id, name: item.name, hero: hero.clone(), tier: item.tier })
            .collect();
        if queue.is_empty() {
            return;
        }
        tracing::info!("item odds: loading {} watched items with none saved", queue.len());
        self.odds_load = Some(OddsLoad { id: random(), total: queue.len(), queue, saved: 0, skipped: 0, current: None });
        self.next_odds_load_item(cx);
    }

    /// Stop the load; what is saved stays saved.
    pub(in crate::ui) fn stop_odds_load(&mut self, cx: &mut Context<Self>) {
        let Some(load) = self.odds_load.take() else { return };
        // The item being asked for is no longer waited on.
        if load.current.is_some() && self.item_odds_pending == load.current {
            self.item_odds_pending = None;
            self.item_odds_asks += 1;
        }
        // Watched items not reached are asked for again on a later board read.
        for wanted in load.queue.iter().chain(&load.current) {
            self.watched_odds_asked.remove(&(wanted.template_id.to_lowercase(), wanted.hero.clone(), wanted.tier.clone()));
        }
        self.show_notice(format!("Stopped loading item odds: {} saved, {} skipped", load.saved, load.skipped), cx);
        cx.notify();
    }

    /// The item being asked for was saved: go on to the next.
    pub(in crate::ui) fn odds_load_item_saved(&mut self, cx: &mut Context<Self>) {
        if let Some(load) = &mut self.odds_load {
            load.saved += 1;
        }
        self.next_odds_load_item_after_gap(cx);
    }

    /// The item being asked for did not load, for `why`: skip it.
    pub(in crate::ui) fn skip_odds_load_item(&mut self, why: &str, cx: &mut Context<Self>) {
        let Some(load) = &mut self.odds_load else { return };
        if let Some(current) = &load.current {
            tracing::warn!("item odds: skipped {} ({}): {why}", current.name, current.hero);
        }
        load.skipped += 1;
        self.item_odds_pending = None;
        self.item_odds_asks += 1;
        self.next_odds_load_item_after_gap(cx);
    }

    /// Wait [`gap`], then ask for the next item; the load ends at once when none are left. Nothing
    /// happens if the load was stopped meanwhile.
    fn next_odds_load_item_after_gap(&mut self, cx: &mut Context<Self>) {
        let Some(load) = &mut self.odds_load else { return };
        if load.queue.is_empty() {
            self.next_odds_load_item(cx);
            return;
        }
        load.current = None;
        let id = load.id;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(gap()).await;
            this.update(cx, |this, cx| {
                if this.odds_load.as_ref().is_some_and(|load| load.id == id && load.current.is_none()) {
                    this.next_odds_load_item(cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Ask for the next item, or end the load when there are none left.
    fn next_odds_load_item(&mut self, cx: &mut Context<Self>) {
        let Some(load) = &mut self.odds_load else { return };
        let Some(next) = load.queue.pop_front() else {
            let (saved, skipped) = (load.saved, load.skipped);
            self.odds_load = None;
            match (saved, skipped) {
                (0, skipped) if skipped > 0 => self.show_error(format!("Item odds failed: none of {skipped} items loaded"), cx),
                (saved, 0) => self.show_success(format!("Item odds loaded: {saved} saved"), cx),
                (saved, skipped) => self.show_success(format!("Item odds loaded: {saved} saved, {skipped} skipped"), cx),
            }
            cx.notify();
            return;
        };
        load.current = Some(next.clone());
        if let Err(why) = self.begin_item_odds_ask(next, cx) {
            // The web view itself is broken: every item would fail the same way.
            self.odds_load = None;
            self.show_error(format!("Could not load item odds: {why}"), cx);
        }
        cx.notify();
    }

    /// The status bar's line while a load runs, with a button that stops it.
    pub(in crate::ui) fn render_odds_load_status(&self, cx: &mut Context<Self>) -> Option<Div> {
        let t = self.theme;
        let load = self.odds_load.as_ref()?;
        Some(
            div()
                .flex()
                .items_center()
                .gap_1()
                .min_w_0()
                .child(widgets::icon(widgets::IconName::Loader, t.subtle).size(px(12.)))
                .child(div().min_w_0().truncate().text_color(rgb(t.text)).child(load.describe()))
                .child(widgets::icon_button(t, "stop-odds-load", div().text_color(rgb(t.subtle)).child("✕")).tooltip(widgets::hint(t, vec!["Stop loading item odds".to_owned()])).on_click(cx.listener(|this, _, _, cx| this.stop_odds_load(cx)))),
        )
    }

    /// Item odds: a button for each hero and one for the neutral items, then the button that loads
    /// the odds of every picked item not saved yet.
    pub(in crate::ui) fn render_odds_load_setting(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let heroes = item_odds::heroes(&self.names.catalog_items());
        let pick = &self.odds_load_pick;
        let running = self.odds_load.is_some();
        let hero_buttons = heroes.into_iter().map(|hero| {
            let on = pick.heroes.contains(&hero);
            self.filter_button(format!("odds-hero-{hero}"), bazaar_tracker::enums::hero_names::display_name(&hero).to_owned(), on).on_click(cx.listener(move |this, _, _, cx| {
                let heroes = &mut this.odds_load_pick.heroes;
                if !heroes.remove(&hero) {
                    heroes.insert(hero.clone());
                }
                cx.notify();
            }))
        });
        let neutral = self.filter_button("odds-hero-neutral".to_owned(), "Neutral".to_owned(), pick.neutral).on_click(cx.listener(|this, _, _, cx| {
            this.odds_load_pick.neutral = !this.odds_load_pick.neutral;
            cx.notify();
        }));
        let ready = !running && !pick.heroes.is_empty();
        let label = if running { "Stop" } else { "Load odds" };
        let (accent, overlay) = (t.accent, t.overlay);
        let button = Button::new("load-item-odds")
            .focusable(false)
            .tab_stop(false)
            .flex_none()
            .px_3()
            .h(px(28.))
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(rgb(if ready || running { accent } else { t.highlight_med }))
            .text_sm()
            .text_color(rgb(if ready || running { t.text } else { t.muted }))
            .when(ready || running, |button| {
                button.cursor_pointer().hover(move |style| style.bg(rgb(overlay))).on_click(cx.listener(|this, _, _, cx| {
                    if this.odds_load.is_some() {
                        this.stop_odds_load(cx);
                    } else {
                        this.start_odds_load(cx);
                    }
                }))
            })
            .child(label);
        let state = match (&self.odds_load, self.names.catalog_items().is_empty()) {
            (Some(load), _) => load.describe(),
            (None, true) => "The card catalog has not loaded yet.".to_owned(),
            (None, false) if pick.heroes.is_empty() => "Pick a hero to load its items' odds.".to_owned(),
            (None, false) => format!("{} odds saved so far.", self.item_odds.len()),
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .border_t_1()
            .border_color(rgb(t.overlay))
            .child(div().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(t.muted)).child("ITEM ODDS"))
            .child(div().flex().flex_wrap().items_center().gap_1().children(hero_buttons).child(neutral))
            .child(div().flex().items_center().justify_between().gap_4().child(div().min_w_0().text_sm().text_color(rgb(t.text)).child(state)).child(button))
            .child(div().text_xs().text_color(rgb(t.muted)).child(
                "Loads, one by one in the background, the odds of each picked hero's items (and the neutral ones, for each hero) as not owned, skipping those already saved and any that do not load.",
            ))
            .children(self.unknown_heroes_note())
    }

    /// The heroes the hero list does not know: the catalog's and the one being played.
    pub(in crate::ui) fn unknown_heroes(&self) -> Vec<String> {
        item_odds::unknown_heroes(&item_odds::heroes(&self.names.catalog_items()), self.live.hero())
    }

    /// A warning naming the heroes the hero list does not know, if there are any.
    fn unknown_heroes_note(&self) -> Option<Div> {
        let t = self.theme;
        let unknown = self.unknown_heroes();
        if unknown.is_empty() {
            return None;
        }
        let names = unknown.join(", ");
        Some(div().text_xs().text_color(rgb(t.gold)).child(format!(
            "Unknown hero name: {names}. It is shown as the game names it; add it to the hero list (hero_names in the enums) to give it its display name."
        )))
    }

    /// Log each hero the hero list does not know, once while the app runs.
    pub(in crate::ui) fn warn_unknown_heroes(&mut self) {
        for hero in self.unknown_heroes() {
            if self.warned_heroes.insert(hero.clone()) {
                tracing::warn!("unknown hero name {hero}: shown as the game names it; add it to hero_names");
            }
        }
    }
}
