//! The window: the title bar and its ☰ menu, the island panels (runs sidebar, main run panel,
//! detail panel and log panel), the status bar, and the command palette, theme picker and settings overlay
//! that float above them.
//!
//! `TrackerApp` holds the state and routes keys; each submodule draws one part of it. Nothing
//! outside this module draws.

pub mod frame;
mod live;
mod log_panel;
mod run;
mod shop_odds;
mod widgets;

use std::sync::Arc;
use std::time::Duration;

use gpui::{App, Context, FocusHandle, FollowMode, KeyBinding, ListAlignment, ListState, ScrollHandle, UniformListScrollHandle, KeyDownEvent, Keystroke, MouseButton, SharedString, Window, actions, div, prelude::*, px, rgb, size};

use crate::overlays::Overlay;
use crate::tracking::run::events::KindFilter;
use crate::overlays::commands::{Command, CommandPalette, Menu, MenuOutcome, Outcome as PaletteOutcome};
use crate::overlays::settings_overlay::{Outcome as SettingsOutcome, SettingsOverlay};
use crate::overlays::shop_odds::{Outcome as ShopOddsOutcome, ShopOddsOverlay};
use crate::overlays::theme_picker::{Outcome as PickerOutcome, ThemePicker};
use crate::shell::art::{self, ArtInfo, Progress as ArtProgress, Report as ArtReport};
use crate::shell::settings::{Settings, SettingsStore, WindowSettings};
use crate::theme::{self, Theme};
use crate::tracking::log::details::Details;
use crate::tracking::live::LiveState;
use crate::tracking::log::rows::{LogBuffer, LogRow};
use crate::tracking::log::narrate::Narrator;
use crate::tracking::names::{ArtIndex, Names};
use crate::tracking::service::Link;
use crate::tracking::session::Session;
use crate::tracking::status::Status;
use bazaar_tracker::catalog::CatalogHandle;
use bazaar_tracker::history::{History, SavedShopOdds};
use bazaardb::ShopOdds;
use shop_odds::OddsMessage;
use bazaar_tracker::log::path as log_path;
use bazaar_tracker::tracker::Notice;

actions!(tracker, [Quit, ToggleSidebar, ToggleDetail, ToggleLog, ToggleCommandPalette, ToggleThemePicker]);

/// A key binding. `widgets::shortcut` draws it, in the platform's own notation.
pub struct Shortcut {
    keys: &'static str,
}

impl Shortcut {
    /// The key presses of this shortcut, one for each step of a chord.
    pub fn keystrokes(&self) -> Vec<Keystroke> {
        self.keys.split(' ').filter_map(|keys| Keystroke::parse(keys).ok()).collect()
    }
}

pub const QUIT_SHORTCUT: Shortcut = Shortcut { keys: "secondary-q" };
pub const PALETTE_SHORTCUT: Shortcut = Shortcut { keys: "secondary-p" };
/// Zed's shortcut for its left dock.
pub const SIDEBAR_SHORTCUT: Shortcut = Shortcut { keys: "secondary-b" };
/// Zed's shortcut for its right dock.
pub const DETAIL_SHORTCUT: Shortcut = Shortcut { keys: "secondary-alt-b" };
/// The shortcut for an editor's terminal or output panel.
pub const LOG_SHORTCUT: Shortcut = Shortcut { keys: "secondary-j" };
/// Zed's shortcut for its theme picker.
pub const THEME_PICKER_SHORTCUT: Shortcut = Shortcut { keys: "secondary-k secondary-t" };

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(QUIT_SHORTCUT.keys, Quit, None),
        KeyBinding::new(PALETTE_SHORTCUT.keys, ToggleCommandPalette, None),
        KeyBinding::new(SIDEBAR_SHORTCUT.keys, ToggleSidebar, None),
        KeyBinding::new(DETAIL_SHORTCUT.keys, ToggleDetail, None),
        KeyBinding::new(LOG_SHORTCUT.keys, ToggleLog, None),
        KeyBinding::new(THEME_PICKER_SHORTCUT.keys, ToggleThemePicker, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// The time now, in milliseconds since the Unix epoch.
fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// How long to wait for the card catalog before the names are given up on.
const CATALOG_PATIENCE: Duration = Duration::from_secs(120);

/// How long a notice stays in the status bar.
const NOTICE_TIME: Duration = Duration::from_secs(2);

pub struct TrackerApp {
    settings: SettingsStore,
    theme: &'static Theme,
    /// The theme last copied into gpui-component's, whose charts and tooltips read their colours
    /// from there.
    kit_theme: Option<&'static str>,
    /// The sidebar width (with its gap) while it is dragged; saved when the drag ends.
    sidebar_width: u32,
    resizing_sidebar: bool,
    /// The Run whose delete button was clicked once: a second click deletes it.
    deleting_run: Option<String>,
    /// Whether the detail island is open. It is not saved: the window opens without it.
    detail_open: bool,
    /// The detail island width while it is dragged; saved when the drag ends.
    detail_width: u32,
    resizing_detail: bool,
    /// The most height of the Live island while it is dragged; saved when the drag ends.
    live_height: u32,
    /// While the strip under the Live island is held: where the pointer was pressed, and the
    /// island's height then.
    resizing_live: Option<(f32, f32)>,
    /// The log panel width while it is dragged; saved when the drag ends.
    log_width: u32,
    resizing_log: bool,
    /// What floats above the window. At most one is open; while one is, typing goes to it.
    overlay: Option<Overlay>,
    /// Shown in the status bar until dismissed.
    error: Option<SharedString>,
    /// Shown in the status bar for `NOTICE_TIME`.
    notice: Option<SharedString>,
    /// Counts notices, so an old notice timer can not hide a newer one.
    notice_serial: u32,
    /// Whether the game is attached.
    status: Status,
    /// The Runs, which is live and which is selected.
    session: Session,
    /// The history the tracker saves to, if it started.
    history: Option<Arc<History>>,
    /// Card names, from the game's catalog once it has loaded.
    names: Names,
    /// What the game shows now.
    live: LiveState,
    /// The rows of the log panel, and what turns notices into them.
    log: LogBuffer,
    narrator: Narrator,
    /// The facts pinned at the top of the log panel.
    details: Details,
    /// The log's rows as laid out: only those in view are drawn. It follows the newest row
    /// unless the player has scrolled up, and again once they scroll back to the bottom.
    log_list: ListState,
    /// Scrolls the Events list, which follows a live Run.
    events_scroll: UniformListScrollHandle,
    /// What the card art folder holds, if it has been extracted.
    art: Option<ArtInfo>,
    /// How far an art extraction has got, while one runs.
    art_run: Option<ArtProgress>,
    /// The shop odds, from the history (or the built-in copy if there is none), whether a refresh
    /// or import is under way, and how the last one went.
    shop_odds: SavedShopOdds,
    shop_odds_busy: bool,
    shop_odds_message: Option<OddsMessage>,
    /// Scroll the Live island, the Run island, the runs list and the detail island, for their
    /// scroll bars.
    live_scroll: ScrollHandle,
    main_scroll: ScrollHandle,
    runs_scroll: ScrollHandle,
    detail_scroll: ScrollHandle,
    /// The sideways scroll of each row of items or skills, by its id, kept between frames.
    sideways_scrolls: std::cell::RefCell<std::collections::HashMap<String, ScrollHandle>>,
    pub focus_handle: FocusHandle,
}

impl TrackerApp {
    /// `startup_error` is a problem found before the window opened (such as an unreadable settings
    /// file). `link` is the tracker service: what it reports, and the history it saves to.
    /// `log_rows` is what `tracing` records, for the log panel.
    pub fn new(settings: SettingsStore, startup_error: Option<String>, link: Link, log_rows: async_channel::Receiver<LogRow>, cx: &mut Context<Self>) -> Self {
        let theme = theme::by_name(Some(&settings.get().theme));
        let sidebar_width = settings.get().window.sidebar_width;
        let log_width = settings.get().window.log_width;
        let detail_width = settings.get().window.detail_width;
        let live_height = settings.get().window.live_height;
        let tracking = link.running();
        let mut startup_error = startup_error;
        let session = match link.history.as_deref().map(Session::load) {
            Some(Ok(session)) => session,
            Some(Err(error)) => {
                tracing::error!("could not list the runs: {error}");
                let problem = format!("Could not list the runs: {error}");
                startup_error = Some(startup_error.map_or_else(|| problem.clone(), |first| format!("{first} {problem}")));
                Session::default()
            }
            None => Session::default(),
        };
        // The card names arrive when the catalog has loaded, off this thread.
        if let Some(profile) = log_path::user_profile() {
            let catalog = CatalogHandle::start(log_path::game_folder_in(&profile));
            cx.spawn(async move |this, cx| {
                let loaded = cx.background_executor().spawn(async move { catalog.wait(CATALOG_PATIENCE) }).await;
                if let Some(loaded) = loaded {
                    this.update(cx, |this, cx| {
                        this.names.set_catalog(loaded);
                        cx.notify();
                    })
                    .ok();
                }
            })
            .detach();
        }
        cx.spawn(async move |this, cx| {
            while let Ok(row) = log_rows.recv().await {
                if this.update(cx, |this, cx| this.push_log(row, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let details = Details::new(link.player_log.clone(), link.history_db.clone(), tracking);
        let notices = link.notices;
        cx.spawn(async move |this, cx| {
            while let Ok(notice) = notices.recv().await {
                if this.update(cx, |this, cx| this.on_notice(&notice, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let art = ArtInfo::load(&art::art_dir(settings.dir()));
        let art_index = Arc::new(ArtIndex::load(&art::art_dir(settings.dir())));
        let shop_odds = load_shop_odds(link.history.as_deref());
        Self {
            settings,
            theme,
            kit_theme: None,
            sidebar_width,
            resizing_sidebar: false,
            deleting_run: None,
            detail_open: false,
            detail_width,
            resizing_detail: false,
            live_height,
            resizing_live: None,
            log_width,
            resizing_log: false,
            overlay: None,
            error: startup_error.map(SharedString::from),
            notice: None,
            notice_serial: 0,
            status: Status::new(tracking),
            session,
            history: link.history,
            names: Names::default().with_art(art_index),
            art,
            art_run: None,
            shop_odds,
            shop_odds_busy: false,
            shop_odds_message: None,
            live: LiveState::default(),
            log: LogBuffer::new(),
            narrator: Narrator::default(),
            details,
            log_list: {
                let list = ListState::new(0, ListAlignment::Top, px(200.));
                list.set_follow_mode(FollowMode::Tail);
                list
            },
            events_scroll: UniformListScrollHandle::new(),
            live_scroll: ScrollHandle::new(),
            main_scroll: ScrollHandle::new(),
            runs_scroll: ScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
            sideways_scrolls: Default::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Something the tracker reported.
    fn on_notice(&mut self, notice: &Notice, cx: &mut Context<Self>) {
        if let Some(error) = self.status.apply(notice) {
            self.show_error(error, cx);
        }
        self.live.apply(notice);
        self.details.apply(notice);
        if let Some(row) = self.narrator.row(notice, &self.names, now_ms()) {
            self.push_log(row, cx);
        }
        if let Some(history) = self.history.clone()
            && let Err(error) = self.session.apply(notice, &history)
        {
            self.show_error(format!("Could not read the history: {error}"), cx);
        }
        // A live Run being watched stays scrolled to its newest Event.
        if self.session.selected_event().is_none() && self.session.selected().is_some_and(|id| self.session.is_live(id)) {
            self.events_scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    fn select_run(&mut self, run_id: &str, cx: &mut Context<Self>) {
        self.deleting_run = None;
        if let Some(history) = self.history.clone()
            && let Err(error) = self.session.select(run_id, &history)
        {
            self.show_error(format!("Could not read the run: {error}"), cx);
        }
        cx.notify();
    }

    /// The delete button of a Run's row: the first click asks, the second deletes the Run.
    fn delete_run(&mut self, run_id: &str, cx: &mut Context<Self>) {
        if self.deleting_run.as_deref() != Some(run_id) {
            self.deleting_run = Some(run_id.to_owned());
        } else {
            self.deleting_run = None;
            if let Some(history) = self.history.clone()
                && let Err(error) = self.session.delete(run_id, &history)
            {
                self.show_error(format!("Could not delete the run: {error}"), cx);
            }
        }
        cx.notify();
    }

    fn toggle_day(&mut self, day: i32, cx: &mut Context<Self>) {
        self.session.toggle_day(day);
        cx.notify();
    }

    fn show_latest(&mut self, cx: &mut Context<Self>) {
        self.session.show_latest();
        cx.notify();
    }

    fn filter_day(&mut self, day: Option<i32>, cx: &mut Context<Self>) {
        self.session.filter_day(day);
        cx.notify();
    }

    fn filter_hour(&mut self, hour: Option<i32>, cx: &mut Context<Self>) {
        self.session.filter_hour(hour);
        cx.notify();
    }

    fn filter_kind(&mut self, kind: KindFilter, cx: &mut Context<Self>) {
        self.session.filter_kind(kind);
        cx.notify();
    }

    fn toggle_event(&mut self, event_id: &str, cx: &mut Context<Self>) {
        self.session.toggle_event(event_id);
        cx.notify();
    }

    fn toggle_stash(&mut self, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window.stash_open = !settings.window.stash_open, cx);
        cx.notify();
    }

    fn toggle_snapshot(&mut self, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window.snapshot_open = !settings.window.snapshot_open, cx);
        cx.notify();
    }

    /// Show `text` in the status bar until the player dismisses it.
    fn show_error(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        tracing::error!("{text}");
        self.error = Some(text);
        cx.notify();
    }

    fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
    }

    /// Show `text` in the status bar for `NOTICE_TIME`, then hide it (or sooner, if a newer notice replaces it).
    fn show_notice(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notice_serial += 1;
        let serial = self.notice_serial;
        self.notice = Some(text.into());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(NOTICE_TIME).await;
            this.update(cx, |this, cx| {
                if this.notice_serial == serial {
                    this.notice = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Change the settings and save them, reporting a failed save in the status bar.
    fn change_settings(&mut self, change: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        if let Err(error) = self.settings.update(change) {
            self.show_error(format!("Could not save settings: {error}"), cx);
        }
    }

    fn save_sidebar_width(&mut self) {
        let width = self.sidebar_width;
        // A failed save is logged; the width is still in use.
        if let Err(error) = self.settings.update(|settings| settings.window.set_sidebar_width(width)) {
            tracing::error!("Could not save settings: {error}");
        }
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window.sidebar_open = !settings.window.sidebar_open, cx);
        cx.notify();
    }

    /// Add a row to the log, and tell its list about the row and any that dropped off the front.
    fn push_log(&mut self, row: LogRow, cx: &mut Context<Self>) {
        let dropped = self.log.push(row);
        if dropped > 0 {
            self.log_list.splice(0..dropped, 0);
        }
        if let Some(last) = self.log.len().checked_sub(1) {
            self.log_list.splice(last..last, 1);
        }
        if self.settings.get().window.log_open {
            cx.notify();
        }
    }

    fn toggle_log(&mut self, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window.log_open = !settings.window.log_open, cx);
        cx.notify();
    }

    fn toggle_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_open = !self.detail_open;
        cx.notify();
    }

    /// Close whatever is open, and open `overlay` instead.
    fn open(&mut self, overlay: Overlay, cx: &mut Context<Self>) {
        self.dismiss(cx);
        self.overlay = Some(overlay);
        cx.notify();
    }

    /// Close whatever is open. Closing the theme picker this way puts back the theme from before it opened.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::ThemePicker(picker)) = self.overlay.take() {
            self.theme = picker.original;
        }
        cx.notify();
    }

    fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        match self.overlay {
            Some(Overlay::Menu(_)) => self.dismiss(cx),
            _ => self.open(Overlay::Menu(Menu), cx),
        }
    }

    fn toggle_palette(&mut self, cx: &mut Context<Self>) {
        match self.overlay {
            Some(Overlay::Palette(_)) => self.dismiss(cx),
            _ => self.open(Overlay::Palette(CommandPalette::default()), cx),
        }
    }

    fn toggle_theme_picker(&mut self, cx: &mut Context<Self>) {
        match self.overlay {
            Some(Overlay::ThemePicker(_)) => self.dismiss(cx),
            _ => self.open(Overlay::ThemePicker(ThemePicker::new(self.theme)), cx),
        }
    }

    /// Run a command from the menu or the palette.
    fn run(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::Settings => self.open(Overlay::Settings(SettingsOverlay), cx),
            Command::ExtractArt => {
                self.open(Overlay::Settings(SettingsOverlay), cx);
                self.extract_art(cx);
            }
            Command::ChangeTheme => self.open(Overlay::ThemePicker(ThemePicker::new(self.theme)), cx),
            Command::ToggleSidebar => {
                self.dismiss(cx);
                self.toggle_sidebar(cx);
            }
            Command::ToggleDetail => {
                self.dismiss(cx);
                self.toggle_detail(cx);
            }
            Command::ToggleLog => {
                self.dismiss(cx);
                self.toggle_log(cx);
            }
            Command::ToggleRunDuration => {
                self.dismiss(cx);
                self.change_settings(|settings| settings.show_run_duration = !settings.show_run_duration, cx);
            }
            Command::ResetLayout => {
                self.dismiss(cx);
                self.reset_layout(window, cx);
            }
            Command::Quit => cx.quit(),
        }
        cx.notify();
    }

    /// Extract the card art from the game into the data folder, unless that is already running.
    /// Progress shows in the settings overlay; the outcome, in the status bar.
    fn extract_art(&mut self, cx: &mut Context<Self>) {
        if self.art_run.is_some() {
            return;
        }
        self.art_run = Some(ArtProgress::default());
        let reports = art::start(self.settings.dir(), &self.settings.get().game_dir);
        let art_dir = art::art_dir(self.settings.dir());
        cx.spawn(async move |this, cx| {
            while let Ok(report) = reports.recv().await {
                let finished = matches!(report, ArtReport::Finished(_));
                let updated = this.update(cx, |this, cx| {
                    match report {
                        ArtReport::Step(step) => {
                            if let Some(progress) = this.art_run.as_mut() {
                                progress.apply(&step);
                            }
                        }
                        ArtReport::Finished(outcome) => {
                            this.art_run = None;
                            this.art = ArtInfo::load(&art_dir);
                            this.names.set_art(Arc::new(ArtIndex::load(&art_dir)));
                            match outcome {
                                Ok(cards) => this.show_notice(format!("Card art ready for {cards} cards"), cx),
                                Err(why) => this.show_error(format!("Could not extract card art: {why}"), cx),
                            }
                        }
                    }
                    cx.notify();
                });
                if finished || updated.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn open_shop_odds(&mut self, cx: &mut Context<Self>) {
        self.shop_odds_message = None;
        self.open(Overlay::ShopOdds(ShopOddsOverlay), cx);
    }

    /// Ask Bazaar DB for the shop odds, off this thread, and save them if they come. Bazaar DB
    /// wants a token only a browser can get, so this is often refused; the pop-up says so.
    fn refresh_shop_odds(&mut self, cx: &mut Context<Self>) {
        if self.shop_odds_busy {
            return;
        }
        self.shop_odds_busy = true;
        self.shop_odds_message = None;
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async { bazaardb::fetch_shop_odds() }).await;
            this.update(cx, |this, cx| {
                this.shop_odds_busy = false;
                match result {
                    Ok(odds) => this.store_shop_odds(odds),
                    Err(error) => this.shop_odds_failed(format!("{error}. Use Import JSON… with the answer saved from your browser.")),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Read the shop odds from a file: the `/api/shop-odds` answer saved from a browser.
    fn import_shop_odds(&mut self, cx: &mut Context<Self>) {
        if self.shop_odds_busy {
            return;
        }
        let chosen = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Import".into()) });
        self.shop_odds_busy = true;
        self.shop_odds_message = None;
        cx.spawn(async move |this, cx| {
            let path = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let result = match path {
                None => None,
                Some(path) => Some(
                    std::fs::read_to_string(&path)
                        .map_err(|error| format!("Could not read {}: {error}", path.display()))
                        .and_then(|text| bazaardb::shop_odds::parse_json(&text).map_err(|error| format!("Could not import {}: {error}", path.display()))),
                ),
            };
            this.update(cx, |this, cx| {
                this.shop_odds_busy = false;
                match result {
                    None => {}
                    Some(Ok(odds)) => this.store_shop_odds(odds),
                    Some(Err(why)) => this.shop_odds_failed(why),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Keep new shop odds, saving them to the history if there is one.
    fn store_shop_odds(&mut self, odds: ShopOdds) {
        if odds.days.is_empty() {
            self.shop_odds_failed("Bazaar DB's answer had no days; the odds were kept as they were.".to_owned());
            return;
        }
        let days = odds.days.len();
        let now = now_ms();
        match self.history.as_deref() {
            Some(history) => {
                if let Err(error) = history.save_shop_odds(&odds, now) {
                    self.shop_odds_failed(format!("Could not save the shop odds: {error}"));
                    return;
                }
                self.shop_odds = load_shop_odds(Some(history));
            }
            None => self.shop_odds = SavedShopOdds { odds, saved_ms: Some(now) },
        }
        self.shop_odds_message = Some(OddsMessage { text: format!("Saved odds for {days} days.").into(), error: false });
    }

    fn shop_odds_failed(&mut self, why: String) {
        tracing::warn!("shop odds: {why}");
        self.shop_odds_message = Some(OddsMessage { text: why.into(), error: true });
    }

    /// Put the panels and the window size back the way they are on first launch.
    fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window = WindowSettings::default(), cx);
        self.sidebar_width = self.settings.get().window.sidebar_width;
        self.log_width = self.settings.get().window.log_width;
        self.detail_width = self.settings.get().window.detail_width;
        self.live_height = self.settings.get().window.live_height;
        self.detail_open = false;
        if !window.is_maximized() && !window.is_fullscreen() {
            let (width, height) = crate::DEFAULT_WINDOW_SIZE;
            window.resize(size(px(width), px(height)));
        }
        self.show_notice("Layout reset", cx);
    }

    /// Keep `theme` and remember it.
    fn choose_theme(&mut self, theme: &'static Theme, cx: &mut Context<Self>) {
        self.overlay = None;
        self.theme = theme;
        self.change_settings(|settings| settings.theme = theme.name.to_string(), cx);
        cx.notify();
    }

    /// A key from the real keyboard, to whatever is open. With nothing open it goes nowhere: the
    /// window has nothing to type into.
    fn key_down(&mut self, key: &str, typed: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        match self.overlay.as_mut() {
            None => {}
            Some(Overlay::Menu(menu)) => {
                if menu.press(key) == MenuOutcome::Close {
                    self.dismiss(cx);
                }
            }
            Some(Overlay::Palette(palette)) => match palette.press(key, typed) {
                PaletteOutcome::Run(command) => {
                    self.overlay = None;
                    self.run(command, window, cx);
                }
                PaletteOutcome::Close => self.dismiss(cx),
                PaletteOutcome::None => {}
            },
            Some(Overlay::ThemePicker(picker)) => match picker.press(key, typed) {
                PickerOutcome::Preview(theme) => self.theme = theme,
                PickerOutcome::Choose(theme) => self.choose_theme(theme, cx),
                PickerOutcome::Restore(theme) => {
                    self.overlay = None;
                    self.theme = theme;
                }
                PickerOutcome::None => {}
            },
            Some(Overlay::Settings(settings)) => {
                if settings.press(key) == SettingsOutcome::Close {
                    self.dismiss(cx);
                }
            }
            Some(Overlay::ShopOdds(odds)) => {
                if odds.press(key) == ShopOddsOutcome::Close {
                    self.dismiss(cx);
                }
            }
        }
        cx.notify();
    }
}

/// The shop odds in the history, which gets the built-in copy first if it has none. Without a
/// history, or if it can not be read, the built-in copy (with no save time).
fn load_shop_odds(history: Option<&History>) -> SavedShopOdds {
    let built_in = || SavedShopOdds { odds: bazaardb::shop_odds::defaults(), saved_ms: None };
    let Some(history) = history else { return built_in() };
    let loaded = history.seed_shop_odds(&bazaardb::shop_odds::defaults(), now_ms()).and_then(|_| history.shop_odds());
    match loaded {
        Ok(saved) => saved,
        Err(error) => {
            tracing::error!("could not read the shop odds: {error}");
            built_in()
        }
    }
}

/// Copy the colours gpui-component's charts and their tooltips read into its theme, so they follow
/// the app's.
fn sync_kit_theme(t: &Theme, cx: &mut App) {
    gpui_component::Theme::update(cx, |kit| {
        let colors = &mut kit.colors;
        colors.background = rgb(t.surface).into();
        colors.foreground = rgb(t.text).into();
        colors.muted_foreground = rgb(t.muted).into();
        colors.border = rgb(t.highlight_med).into();
        colors.popover = rgb(t.surface).into();
        colors.popover_foreground = rgb(t.text).into();
        colors.chart_grid = rgb(t.overlay).into();
    });
}

impl Render for TrackerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        if self.kit_theme != Some(t.name) {
            self.kit_theme = Some(t.name);
            sync_kit_theme(t, cx);
        }
        let (sidebar_open, detail_open, log_open) = (self.settings.get().window.sidebar_open, self.detail_open, self.settings.get().window.log_open);

        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ToggleCommandPalette, _, cx| this.toggle_palette(cx)))
            .on_action(cx.listener(|this, _: &ToggleThemePicker, _, cx| this.toggle_theme_picker(cx)))
            // While an overlay is open, the panel shortcuts leave the window behind it alone.
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                if this.overlay.is_none() {
                    this.toggle_sidebar(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleDetail, _, cx| {
                if this.overlay.is_none() {
                    this.toggle_detail(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleLog, _, cx| {
                if this.overlay.is_none() {
                    this.toggle_log(cx);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let keystroke = &event.keystroke;
                let modifiers = &keystroke.modifiers;
                if modifiers.platform || modifiers.control || modifiers.alt {
                    return;
                }
                this.key_down(&keystroke.key, keystroke.key_char.as_deref(), window, cx);
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                this.drag_sidebar(event.position.x, cx);
                this.drag_log(event.position.x, window.viewport_size().width, cx);
                this.drag_detail(event.position.x, window.viewport_size().width, cx);
                this.drag_live(event.position.y, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_sidebar_drag(cx);
                    this.end_log_drag(cx);
                    this.end_detail_drag(cx);
                    this.end_live_drag(cx);
                }),
            )
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(t.background))
            .text_color(rgb(t.text))
            .child(frame::title_bar::render(t, self.render_menu_button(cx), window))
            // The panels are islands: rounded and floating on the window, with a gap between them.
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .px_2()
                    .pb_1()
                    .when(sidebar_open, |row| row.child(self.render_sidebar(cx)).child(self.render_sidebar_handle(cx)))
                    .child(div().flex().flex_1().min_w_0().h_full().child(self.render_main_panel(cx)))
                    .when(detail_open, |row| row.child(self.render_detail_handle(cx)).child(self.render_detail(cx)))
                    .when(log_open, |row| row.child(self.render_log_handle(cx)).child(self.render_log_panel(cx))),
            )
            .child(self.render_status_bar(cx))
            // While anything is open, a click anywhere else closes it (it draws above this).
            // Closing the theme picker this way puts the old theme back.
            .when(self.overlay.is_some(), |root| root.child(div().id("backdrop").absolute().inset_0().occlude().on_click(cx.listener(|this, _, _, cx| this.dismiss(cx)))))
            .map(|root| match &self.overlay {
                Some(Overlay::Palette(palette)) => root.child(self.render_palette(palette, cx)),
                Some(Overlay::ThemePicker(picker)) => root.child(self.render_theme_picker(picker, cx)),
                Some(Overlay::Settings(_)) => root.child(self.render_settings(cx)),
                Some(Overlay::ShopOdds(_)) => root.child(self.render_shop_odds_overlay(cx)),
                _ => root,
            })
    }
}
