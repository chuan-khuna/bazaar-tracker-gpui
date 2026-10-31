//! The window: the title bar and its ☰ menu, the three island panels (runs sidebar, main run panel
//! and detail panel), the status bar, and the command palette, theme picker and settings overlay
//! that float above them.
//!
//! `TrackerApp` holds the state and routes keys; each submodule draws one part of it. Nothing
//! outside this module draws.

mod detail;
mod main_panel;
mod menu;
mod palette;
mod settings;
mod sidebar;
mod status_bar;
mod theme_picker;
pub mod title_bar;
mod widgets;

use std::time::Duration;

use gpui::{App, Context, FocusHandle, KeyBinding, KeyDownEvent, Keystroke, MouseButton, SharedString, Window, actions, div, prelude::*, px, rgb, size};

use crate::overlays::Overlay;
use crate::overlays::commands::{Command, CommandPalette, Menu, MenuOutcome, Outcome as PaletteOutcome};
use crate::overlays::settings::{Outcome as SettingsOutcome, SettingsOverlay};
use crate::overlays::theme_picker::{Outcome as PickerOutcome, ThemePicker};
use crate::shell::settings::{Settings, SettingsStore, WindowSettings};
use crate::theme::{self, Theme};

actions!(tracker, [Quit, ToggleSidebar, ToggleDetail, ToggleCommandPalette, ToggleThemePicker]);

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
/// Zed's shortcut for its theme picker.
pub const THEME_PICKER_SHORTCUT: Shortcut = Shortcut { keys: "secondary-k secondary-t" };

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(QUIT_SHORTCUT.keys, Quit, None),
        KeyBinding::new(PALETTE_SHORTCUT.keys, ToggleCommandPalette, None),
        KeyBinding::new(SIDEBAR_SHORTCUT.keys, ToggleSidebar, None),
        KeyBinding::new(DETAIL_SHORTCUT.keys, ToggleDetail, None),
        KeyBinding::new(THEME_PICKER_SHORTCUT.keys, ToggleThemePicker, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// How long a notice stays in the status bar.
const NOTICE_TIME: Duration = Duration::from_secs(2);

pub struct TrackerApp {
    settings: SettingsStore,
    theme: &'static Theme,
    /// The sidebar width (with its gap) while it is dragged; saved when the drag ends.
    sidebar_width: u32,
    resizing_sidebar: bool,
    /// What floats above the window. At most one is open; while one is, typing goes to it.
    overlay: Option<Overlay>,
    /// Shown in the status bar until dismissed.
    error: Option<SharedString>,
    /// Shown in the status bar for `NOTICE_TIME`.
    notice: Option<SharedString>,
    /// Counts notices, so an old notice timer can not hide a newer one.
    notice_serial: u32,
    pub focus_handle: FocusHandle,
}

impl TrackerApp {
    /// `startup_error` is a problem found before the window opened (such as an unreadable settings file).
    pub fn new(settings: SettingsStore, startup_error: Option<String>, cx: &mut Context<Self>) -> Self {
        let theme = theme::by_name(Some(&settings.get().theme));
        let sidebar_width = settings.get().window.sidebar_width;
        Self {
            settings,
            theme,
            sidebar_width,
            resizing_sidebar: false,
            overlay: None,
            error: startup_error.map(SharedString::from),
            notice: None,
            notice_serial: 0,
            focus_handle: cx.focus_handle(),
        }
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
        let (sidebar_open, detail_open) = (self.settings.get().window.sidebar_open, self.settings.get().window.detail_open);

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
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let keystroke = &event.keystroke;
                let modifiers = &keystroke.modifiers;
                if modifiers.platform || modifiers.control || modifiers.alt {
                    return;
                }
                this.key_down(&keystroke.key, keystroke.key_char.as_deref(), window, cx);
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| this.drag_sidebar(event.position.x, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.end_sidebar_drag(cx)))
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
                    .when(sidebar_open, |row| row.child(self.render_sidebar()).child(self.render_sidebar_handle(cx)))
                    .child(self.render_main_panel())
                    .when(detail_open, |row| row.child(div().flex_none().w(sidebar::GAP)).child(self.render_detail())),
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
