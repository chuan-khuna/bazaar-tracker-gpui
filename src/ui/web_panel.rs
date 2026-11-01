//! The Bazaar DB panel, a debug view: Bazaar DB's shop odds page in a web view
//! (`bazaardb::webview`), and the API replies the page gets back.
//!
//! Shop odds that come back are saved, as an import is. The view sits in a popup above the window,
//! so this keeps it over the panel's body each frame, and out of the way of an overlay.

use std::collections::VecDeque;
use std::rc::Rc;

use bazaardb::webview::{self, Api, Capture, Rect, WebView};
use gpui::{Bounds, Context, Pixels, SharedString, Window, canvas, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::widgets::{self, IconName};

/// How long a shop odds refresh waits for the page to send them.
const REFRESH_PATIENCE: std::time::Duration = std::time::Duration::from_secs(30);

/// How many replies the panel keeps.
const KEPT: usize = 20;

/// The panel's width, in pixels.
const WIDTH: f32 = 560.;

/// One reply, as the panel lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub ok: bool,
    /// The API, and what was asked of it.
    pub label: String,
    /// How it went: the size of the answer, or why it failed.
    pub outcome: String,
    /// The start of the answer, when it is kept unparsed.
    pub preview: String,
}

impl Seen {
    pub fn of(capture: &Capture) -> Self {
        let preview = |body: &str| body.chars().take(160).collect::<String>();
        match capture {
            Capture::ShopOdds(odds) => Seen { ok: true, label: "shop-odds".into(), outcome: format!("{} days", odds.days.len()), preview: String::new() },
            Capture::ItemOdds(odds) => Seen {
                ok: true,
                label: format!("item-odds · {} · {} · {}", odds.hero, odds.tier, odds.item),
                outcome: format!("{} B", odds.body.len()),
                preview: preview(&odds.body),
            },
            Capture::Runs(runs) => Seen { ok: true, label: format!("run · {}", runs.query), outcome: format!("{} B", runs.body.len()), preview: preview(&runs.body) },
            Capture::Failed { api, why, .. } => Seen { ok: false, label: api_name(*api).into(), outcome: why.clone(), preview: String::new() },
        }
    }
}

fn api_name(api: Api) -> &'static str {
    match api {
        Api::ShopOdds => "shop-odds",
        Api::ItemOdds => "item-odds",
        Api::Runs => "run",
    }
}

/// The panel's state.
pub struct WebPanel {
    pub open: bool,
    view: Option<Rc<WebView>>,
    /// Why the web view could not be made, if it could not.
    failed: Option<SharedString>,
    /// The newest reply first.
    seen: VecDeque<Seen>,
    sender: async_channel::Sender<Capture>,
    /// Whether the window's moves are watched, so the view follows the window.
    following: bool,
    /// The view is wanted before the panel has opened: a refresh asked for it.
    view_wanted: bool,
    /// The shop odds refresh under way, by its number, so an old timeout can not end a newer one.
    refreshing: Option<u32>,
    refreshes: u32,
}

impl WebPanel {
    /// `sender` takes each reply the page gets.
    pub fn new(sender: async_channel::Sender<Capture>) -> Self {
        Self { open: false, view: None, failed: None, seen: VecDeque::new(), sender, following: false, view_wanted: false, refreshing: None, refreshes: 0 }
    }

    pub fn push(&mut self, seen: Seen) {
        self.seen.push_front(seen);
        self.seen.truncate(KEPT);
    }

    /// Whether the view should be made now, though the panel is shut.
    pub fn wants_view(&self) -> bool {
        self.view_wanted && self.view.is_none() && self.failed.is_none()
    }
}

/// `bounds`, in the window's logical pixels, as physical pixels.
fn physical(bounds: Bounds<Pixels>, scale: f32) -> Rect {
    let px = |value: Pixels| (f32::from(value) * scale).round() as i32;
    Rect { x: px(bounds.origin.x), y: px(bounds.origin.y), width: px(bounds.size.width), height: px(bounds.size.height) }
}

impl TrackerApp {
    pub(super) fn toggle_web(&mut self, cx: &mut Context<Self>) {
        self.web.open = !self.web.open;
        if !self.web.open
            && let Some(view) = &self.web.view
        {
            view.hide();
        }
        cx.notify();
    }

    /// Get the shop odds by loading Bazaar DB's shop odds page in the web view, which asks for
    /// them as it loads; [`Self::on_web_capture`] saves them when they come. If they do not come in
    /// time, the page is likely showing Cloudflare's check, which the panel shows.
    pub(super) fn refresh_shop_odds_in_web(&mut self, cx: &mut Context<Self>) {
        if self.shop_odds_busy {
            return;
        }
        if let Some(failed) = &self.web.failed {
            let why = failed.to_string();
            self.shop_odds_failed(why);
            cx.notify();
            return;
        }
        self.shop_odds_busy = true;
        self.shop_odds_message = None;
        self.web.refreshes += 1;
        let refresh = self.web.refreshes;
        self.web.refreshing = Some(refresh);
        match &self.web.view {
            Some(view) => {
                if let Err(error) = view.load_url(webview::shop_odds::PAGE) {
                    self.end_web_refresh();
                    self.shop_odds_failed(format!("Could not load Bazaar DB's page: {error}"));
                }
            }
            // The view opens on the shop odds page; it is made at the next frame.
            None => self.web.view_wanted = true,
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REFRESH_PATIENCE).await;
            this.update(cx, |this, cx| {
                if this.web.refreshing == Some(refresh) {
                    this.end_web_refresh();
                    this.shop_odds_failed("Bazaar DB's page did not send the odds in time. Open the Bazaar DB panel (the globe in the status bar): it may want you to tick its check.".to_owned());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn end_web_refresh(&mut self) {
        self.web.refreshing = None;
        self.shop_odds_busy = false;
    }

    /// A reply the page got. Shop odds are saved as an import is, and end a refresh.
    pub(super) fn on_web_capture(&mut self, capture: Capture, cx: &mut Context<Self>) {
        let ends_refresh = self.web.refreshing.is_some() && capture.api() == Api::ShopOdds;
        if ends_refresh {
            self.end_web_refresh();
        }
        let seen = Seen::of(&capture);
        if seen.ok {
            tracing::info!("Bazaar DB {}: {}", seen.label, seen.outcome);
        } else {
            tracing::warn!("Bazaar DB {}: {}", seen.label, seen.outcome);
        }
        self.web.push(seen);
        match capture {
            Capture::ShopOdds(odds) => self.store_shop_odds(odds),
            Capture::Failed { why, .. } if ends_refresh => self.shop_odds_failed(why),
            _ => {}
        }
        cx.notify();
    }

    /// Make the web view the first time the panel opens. It is kept while the panel is shut, so the
    /// page and its Cloudflare pass are not lost.
    pub(super) fn ensure_web_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.web.following {
            self.web.following = true;
            cx.observe_window_bounds(window, |_, _, cx| cx.notify()).detach();
        }
        if self.web.view.is_some() || self.web.failed.is_some() {
            return;
        }
        let data_dir = self.settings.dir().join("webview");
        let sender = self.web.sender.clone();
        match WebView::open(window, data_dir, webview::shop_odds::PAGE, move |capture| {
            sender.try_send(capture).ok();
        }) {
            Ok(view) => self.web.view = Some(Rc::new(view)),
            Err(error) => {
                tracing::error!("could not open the Bazaar DB panel: {error}");
                self.web.failed = Some(format!("Could not open the web view: {error}").into());
            }
        }
    }

    pub(super) fn render_web_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        self.ensure_web_view(window, cx);
        let t = self.theme;
        // The view draws above everything, so it steps aside while an overlay is open.
        let shown = self.overlay.is_none();
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .px_2()
            .h(px(30.))
            .child(div().flex_1().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child("Bazaar DB · shop odds"))
            .child(widgets::icon_button(t, "web-reload", widgets::icon(IconName::RefreshCw, t.subtle)).on_click(cx.listener(|this, _, _, _| {
                if let Some(view) = &this.web.view
                    && let Err(error) = view.reload()
                {
                    tracing::warn!("could not reload the Bazaar DB panel: {error}");
                }
            })))
            .child(widgets::icon_button(t, "web-devtools", widgets::icon(IconName::SquareTerminal, t.subtle)).on_click(cx.listener(|this, _, _, _| {
                if let Some(view) = &this.web.view {
                    view.open_devtools();
                }
            })));
        let body = div().flex_1().min_h_0().mx_2().rounded_md().bg(rgb(t.overlay)).map(|body| match (&self.web.failed, self.web.view.clone()) {
            (Some(failed), _) => body.p_2().text_color(rgb(t.error)).child(failed.clone()),
            (None, Some(view)) => body.child(
                canvas(
                    move |bounds, window, _| {
                        if shown {
                            view.place(physical(bounds, window.scale_factor()));
                        } else {
                            view.hide();
                        }
                    },
                    |_, _, _, _| {},
                )
                .size_full(),
            ),
            (None, None) => body,
        });
        let seen = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_0p5()
            .p_2()
            .max_h(px(180.))
            .overflow_hidden()
            .text_xs()
            .font_family("Consolas")
            .child(div().text_color(rgb(t.muted)).child(format!("API replies: {}", self.web.seen.len())))
            .when(self.web.seen.is_empty(), |list| list.child(div().text_color(rgb(t.subtle)).child("Replies the page gets show here; pick an item for its odds.")))
            .children(self.web.seen.iter().take(6).map(|seen| {
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().text_color(rgb(if seen.ok { t.success } else { t.error })).child(if seen.ok { "ok" } else { "failed" }))
                            .child(div().flex_1().min_w_0().truncate().text_color(rgb(t.text)).child(seen.label.clone()))
                            .child(div().flex_none().text_color(rgb(t.subtle)).child(seen.outcome.clone())),
                    )
                    .when(!seen.preview.is_empty(), |row| row.child(div().text_color(rgb(t.subtle)).truncate().child(seen.preview.clone())))
            }));
        widgets::island(t).flex_none().w(px(WIDTH)).h_full().flex().flex_col().child(header).child(body).child(seen)
    }
}

#[cfg(test)]
#[path = "web_panel_tests.rs"]
mod tests;
