//! The window: the title bar and its ☰ menu, the island panels (runs sidebar, main run panel,
//! detail panel and log panel), the status bar, and the command palette, theme picker and settings overlay
//! that float above them.
//!
//! `TrackerApp` holds the state and routes keys; each submodule draws one part of it. Nothing
//! outside this module draws.

mod board;
mod detail;
mod events;
mod live;
mod log_panel;
mod main_panel;
mod menu;
mod palette;
mod run_header;
mod settings;
mod sidebar;
mod status_bar;
mod theme_picker;
pub mod title_bar;
mod widgets;

use std::sync::Arc;
use std::time::Duration;

use gpui::{App, Context, FocusHandle, ScrollHandle, KeyBinding, KeyDownEvent, Keystroke, MouseButton, SharedString, Window, actions, div, prelude::*, px, rgb, size};

use crate::overlays::Overlay;
use crate::overlays::commands::{Command, CommandPalette, Menu, MenuOutcome, Outcome as PaletteOutcome};
use crate::overlays::settings::{Outcome as SettingsOutcome, SettingsOverlay};
use crate::overlays::theme_picker::{Outcome as PickerOutcome, ThemePicker};
use crate::shell::settings::{Settings, SettingsStore, WindowSettings};
use crate::theme::{self, Theme};
use crate::tracking::details::Details;
use crate::tracking::live::LiveState;
use crate::tracking::log::{LogBuffer, LogRow};
use crate::tracking::narrate::Narrator;
use crate::tracking::names::Names;
use crate::tracking::service::Link;
use crate::tracking::session::Session;
use crate::tracking::status::Status;
use bazaar_tracker::catalog::CatalogHandle;
use bazaar_tracker::history::History;
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
    /// The sidebar width (with its gap) while it is dragged; saved when the drag ends.
    sidebar_width: u32,
    resizing_sidebar: bool,
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
    log_scroll: ScrollHandle,
    /// Scrolls the Events list, which follows a live Run.
    events_scroll: ScrollHandle,
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
                        this.names = Names::new(loaded);
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
        Self {
            settings,
            theme,
            sidebar_width,
            resizing_sidebar: false,
            log_width,
            resizing_log: false,
            overlay: None,
            error: startup_error.map(SharedString::from),
            notice: None,
            notice_serial: 0,
            status: Status::new(tracking),
            session,
            history: link.history,
            names: Names::default(),
            live: LiveState::default(),
            log: LogBuffer::new(),
            narrator: Narrator::default(),
            details,
            log_scroll: ScrollHandle::new(),
            events_scroll: ScrollHandle::new(),
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
        if let Some(history) = self.history.clone()
            && let Err(error) = self.session.select(run_id, &history)
        {
            self.show_error(format!("Could not read the run: {error}"), cx);
        }
        cx.notify();
    }

    fn toggle_day(&mut self, day: i32, cx: &mut Context<Self>) {
        self.session.toggle_day(day);
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

    /// Add a row to the log. The log stays on its newest row if it was there, and is left alone if the
    /// player has scrolled up.
    fn push_log(&mut self, row: LogRow, cx: &mut Context<Self>) {
        let at_bottom = self.log_at_bottom();
        self.log.push(row);
        if at_bottom {
            self.log_scroll.scroll_to_bottom();
        }
        if self.settings.get().window.log_open {
            cx.notify();
        }
    }

    /// Whether the log is showing its newest row (also true while nothing has been laid out yet).
    fn log_at_bottom(&self) -> bool {
        let furthest = self.log_scroll.max_offset().y;
        furthest <= px(1.) || -self.log_scroll.offset().y >= furthest - px(4.)
    }

    fn toggle_log(&mut self, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window.log_open = !settings.window.log_open, cx);
        cx.notify();
    }

    fn toggle_detail(&mut self, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window.detail_open = !settings.window.detail_open, cx);
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

    /// Put the panels and the window size back the way they are on first launch.
    fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_settings(|settings| settings.window = WindowSettings::default(), cx);
        self.sidebar_width = self.settings.get().window.sidebar_width;
        self.log_width = self.settings.get().window.log_width;
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
        }
        cx.notify();
    }
}

impl Render for TrackerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let (sidebar_open, detail_open, log_open) = (self.settings.get().window.sidebar_open, self.settings.get().window.detail_open, self.settings.get().window.log_open);

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
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.end_sidebar_drag(cx);
                    this.end_log_drag(cx);
                }),
            )
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(t.background))
            .text_color(rgb(t.text))
            .child(title_bar::render(t, self.render_menu_button(cx), window))
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
                    .when(detail_open, |row| row.child(div().flex_none().w(sidebar::GAP)).child(self.render_detail()))
                    .when(log_open, |row| row.child(self.render_log_handle(cx)).child(self.render_log_panel())),
            )
            .child(self.render_status_bar(cx))
            // While anything is open, a click anywhere else closes it (it draws above this).
            // Closing the theme picker this way puts the old theme back.
            .when(self.overlay.is_some(), |root| root.child(div().id("backdrop").absolute().inset_0().occlude().on_click(cx.listener(|this, _, _, cx| this.dismiss(cx)))))
            .map(|root| match &self.overlay {
                Some(Overlay::Palette(palette)) => root.child(self.render_palette(palette, cx)),
                Some(Overlay::ThemePicker(picker)) => root.child(self.render_theme_picker(picker, cx)),
                Some(Overlay::Settings(_)) => root.child(self.render_settings(cx)),
                _ => root,
            })
    }
}
