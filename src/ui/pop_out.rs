//! The pop-out: the Economy, the Shop odds, the Item odds and the Best shops, each shown or hidden
//! by a chip at its top, one under another, in a small window over a blur of what is behind it,
//! which stays above every other window, the game's too. Its top strip shows the game's day and
//! hour and drags it, its ✕ shuts it, and the grip in its bottom right corner resizes it. While the main window is in front the pop-out is closed; once the main
//! window is left it opens again where it was, without taking the keyboard from the game.

use gpui::{
    App, Bounds, Context, CursorStyle, Div, Entity, MouseButton, Pixels, Window, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowHandle, WindowKind, WindowOptions, div, linear_color_stop, linear_gradient, point, prelude::*, px, rgb, rgba,
    size,
};

use crate::shell::settings::PopOutSettings;
use crate::tracking::live::Live;
use crate::tracking::status::Connection;
use crate::ui::TrackerApp;
use crate::ui::widgets::{self, IconName};

/// The pop-out's size when it first opens.
const SIZE: (f32, f32) = (420., 520.);
/// The least it can be resized to.
const MIN_SIZE: (f32, f32) = (300., 160.);
/// How far it first opens from the top right corner of the screen.
const MARGIN: f32 = 24.;
/// The height of the strip that drags it.
const BAR_HEIGHT: f32 = 26.;
/// The side of the resize grip.
const GRIP: f32 = 14.;
/// With frost, how much thinner the wash is at the bottom than at the top, and the pale rim around
/// it (white, as 0xRRGGBBAA).
const FROST_FADE: f32 = 0.7;
const FROST_RIM: u32 = 0xFFFF_FF55;

/// Whether the pop-out is wanted, and if so whether it is open.
#[derive(Default)]
pub(in crate::ui) enum PopOutState {
    #[default]
    Off,
    /// Wanted, but closed while the main window is in front; it opens again at these bounds.
    Hidden(Option<Bounds<Pixels>>),
    /// Asked to open, and not open yet.
    Opening,
    Shown(WindowHandle<PopOut>),
}

impl TrackerApp {
    /// Closes the pop-out while `window`, the main window, is in front, and opens it again once it
    /// is not.
    pub fn follow_main_window(window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_activation(window, |this, window, cx| this.main_window_activated(window.is_window_active(), cx)).detach();
    }

    fn main_window_activated(&mut self, active: bool, cx: &mut Context<Self>) {
        match (&self.pop_out, active) {
            (PopOutState::Shown(handle), true) => {
                let bounds = handle
                    .update(cx, |_, window, _| {
                        let bounds = window.bounds();
                        window.remove_window();
                        bounds
                    })
                    .ok();
                self.pop_out = PopOutState::Hidden(bounds);
            }
            (PopOutState::Hidden(bounds), false) => self.open_pop_out(*bounds, false, cx),
            _ => {}
        }
    }

    /// The button at the top of the Live section that opens the pop-out.
    pub(in crate::ui) fn pop_out_button(&self, cx: &mut Context<Self>) -> gpui_base::Button {
        let t = self.theme;
        widgets::icon_button(t, "pop-out", widgets::icon(IconName::PictureInPicture2, t.subtle))
            .tooltip(widgets::hint(t, vec!["Pop the Economy and odds out over the game".into()]))
            .on_click(cx.listener(|this, _, _, cx| this.show_pop_out(cx)))
    }

    /// The pop-out button: opens the pop-out in front, where it last was.
    pub(in crate::ui) fn show_pop_out(&mut self, cx: &mut Context<Self>) {
        match &self.pop_out {
            PopOutState::Off => self.open_pop_out(None, true, cx),
            PopOutState::Hidden(bounds) => self.open_pop_out(*bounds, true, cx),
            PopOutState::Opening => {}
            PopOutState::Shown(handle) => {
                handle.update(cx, |_, window, _| window.activate_window()).ok();
            }
        }
    }

    /// Opens the pop-out once the current update is over: opening it changes which window is in
    /// front, which calls back into this entity.
    fn open_pop_out(&mut self, bounds: Option<Bounds<Pixels>>, focus: bool, cx: &mut Context<Self>) {
        self.pop_out = PopOutState::Opening;
        let app = cx.entity();
        cx.defer(move |cx| {
            let bounds = bounds.unwrap_or_else(|| first_bounds(cx));
            let frost = app.read(cx).settings.get().pop_out.frost;
            let opened = cx.open_window(options(bounds, focus, frost), |window, cx| {
                let closing = app.clone();
                // Alt+F4 shuts it as its ✕ does.
                window.on_window_should_close(cx, move |_, cx| {
                    closing.update(cx, |this, _| this.pop_out = PopOutState::Off);
                    true
                });
                cx.new(|cx| PopOut::new(app.clone(), cx))
            });
            app.update(cx, |this, cx| {
                this.pop_out = match opened {
                    Ok(handle) => PopOutState::Shown(handle),
                    Err(error) => {
                        tracing::warn!("could not open the pop-out: {error}");
                        PopOutState::Off
                    }
                };
                cx.notify();
            });
        });
    }
}

fn options(bounds: Bounds<Pixels>, focus: bool, frost: bool) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus,
        show: true,
        // Above every other window, and not on the taskbar.
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: false,
        is_minimizable: false,
        window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
        window_background: background(frost),
        ..Default::default()
    }
}

/// Hands a drag from the bottom right corner to Windows' own sizing loop, which follows the pointer
/// smoothly and keeps to the least size. It is posted, so it starts once gpui has handled the press.
#[cfg(windows)]
fn start_resize(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, SC_SIZE, WM_SYSCOMMAND};
    /// `WMSZ_BOTTOMRIGHT`: which corner `SC_SIZE` moves.
    const BOTTOM_RIGHT: usize = 8;
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
    unsafe {
        ReleaseCapture();
        PostMessageW(handle.hwnd.get() as _, WM_SYSCOMMAND, SC_SIZE as usize + BOTTOM_RIGHT, 0);
    }
}

#[cfg(not(windows))]
fn start_resize(_window: &Window) {}

/// Blurred behind for frosted glass, else plainly see-through.
fn background(frost: bool) -> WindowBackgroundAppearance {
    if frost { WindowBackgroundAppearance::Blurred } else { WindowBackgroundAppearance::Transparent }
}

/// The top right corner of the main screen.
fn first_bounds(cx: &App) -> Bounds<Pixels> {
    let size = size(px(SIZE.0), px(SIZE.1));
    let origin = match cx.primary_display() {
        Some(display) => {
            let screen = display.bounds();
            point(screen.right() - size.width - px(MARGIN), screen.top() + px(MARGIN))
        }
        None => point(px(MARGIN), px(MARGIN)),
    };
    Bounds::new(origin, size)
}

impl TrackerApp {
    /// The game's day and hour, while a Run is played.
    fn pop_out_time(&self) -> Option<String> {
        match self.live.view(self.status.connection == Connection::Tracking, self.session.live().is_some(), &self.names) {
            Live::View(view) => view.mine.time,
            Live::Placeholder(_) => None,
        }
    }

    /// A chip for each section, which shows or hides it, then the sections shown, one under
    /// another, as wide as the pop-out.
    fn render_pop_out_body(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let shown = self.settings.get().pop_out;
        // A section with its title, then its content or, without any, a note.
        let section = |title: &'static str, content: Option<Div>, text: &'static str| {
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .px_3()
                .py_2()
                .child(widgets::section_header([widgets::section_title(t, title)], []))
                .map(|section| match content {
                    Some(content) => section.child(content),
                    None => section.child(div().text_xs().text_color(rgb(t.muted)).child(text)),
                })
        };
        let empty = |title: &'static str, text: &'static str| section(title, None, text);
        let chips: [(&str, bool, fn(&mut PopOutSettings) -> &mut bool); 4] = [
            ("Economy", shown.economy, |s| &mut s.economy),
            ("Shop odds", shown.shop_odds, |s| &mut s.shop_odds),
            ("Item odds", shown.item_odds, |s| &mut s.item_odds),
            ("Best shops", shown.best_shops, |s| &mut s.best_shops),
        ];
        let chips = div().flex().flex_wrap().gap_1().px_3().pb_1().children(chips.map(|(label, on, field)| {
            self.filter_button(format!("pop-out-{label}"), label.to_owned(), on).on_click(cx.listener(move |this, _, _, cx| {
                this.change_settings(|settings| *field(&mut settings.pop_out) = !on, cx);
                cx.notify();
            }))
        }));
        let attached = self.status.connection == Connection::Tracking;
        let view = match self.live.view(attached, self.session.live().is_some(), &self.names) {
            Live::View(view) => Some(view),
            Live::Placeholder(_) => None,
        };
        let mut sections: Vec<Div> = Vec::new();
        if shown.economy {
            // The Economy grows sideways in a row of its own.
            sections.push(div().flex().w_full().child(self.render_economy().unwrap_or_else(|| empty("ECONOMY", "No gold to chart yet."))));
        }
        if shown.shop_odds {
            sections.push(self.upcoming_odds_column().w_full());
        }
        if shown.item_odds {
            let table = view.as_ref().and_then(|view| self.choice_odds_grid(view)).map(|grid| self.odds_table("item-odds", &grid, true, None, cx));
            sections.push(section("ITEM ODDS", table, "Shown at an encounter choice or in a merchant's shop, for items with saved odds."));
        }
        if shown.best_shops {
            let table = view.as_ref().and_then(|view| self.best_shops_grid(view)).map(|grid| self.odds_table("best-shops", &grid, false, None, cx));
            sections.push(section("BEST SHOPS", table, "Shown in a Run, for items with saved odds."));
        }
        let rule = || div().flex_none().h(px(1.)).mx_3().bg(rgb(t.overlay));
        let mut body = div().flex().flex_col().w_full().child(chips);
        for (index, section) in sections.into_iter().enumerate() {
            if index > 0 {
                body = body.child(rule());
            }
            body = body.child(section);
        }
        body
    }
}

/// The pop-out window's view: it draws from the main window's [`TrackerApp`] and redraws with it.
pub(in crate::ui) struct PopOut {
    app: Entity<TrackerApp>,
    /// Whether the window is blurred behind now, so a change in the setting is applied once.
    frost: Option<bool>,
}

impl PopOut {
    fn new(app: Entity<TrackerApp>, cx: &mut Context<Self>) -> Self {
        cx.observe(&app, |_, _, cx| cx.notify()).detach();
        Self { app, frost: None }
    }
}

impl Render for PopOut {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = cx.listener(|this, _, window, cx| {
            this.app.update(cx, |app, _| app.pop_out = PopOutState::Off);
            window.remove_window();
        });
        let (t, time, body, look) = self.app.update(cx, |app, cx| (app.theme, app.pop_out_time(), app.render_pop_out_body(cx), app.settings.get().pop_out));
        if self.frost != Some(look.frost) {
            self.frost = Some(look.frost);
            window.set_background_appearance(background(look.frost));
        }
        let bar = div()
            .id("pop-out-bar")
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(BAR_HEIGHT))
            .pl_3()
            .pr_1()
            .window_control_area(WindowControlArea::Drag)
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(if time.is_some() { t.text } else { t.muted }))
                    .child(time.unwrap_or_else(|| "Not in a run".into())),
            )
            // Its own area, or the strip under it would take the click as a drag.
            .child(div().occlude().child(widgets::icon_button(t, "pop-out-close", widgets::icon(IconName::Close, t.subtle)).on_click(close)));
        let grip = div()
            .id("pop-out-grip")
            .absolute()
            .right_0()
            .bottom_0()
            .size(px(GRIP))
            .occlude()
            .cursor(CursorStyle::ResizeUpLeftDownRight)
            .child(div().absolute().right(px(3.)).bottom(px(3.)).size(px(6.)).border_r_2().border_b_2().border_color(rgb(t.subtle)))
            .on_mouse_down(MouseButton::Left, |_, window, _| start_resize(window));
        // A wash of the theme's background as opaque as the setting asks. With frost it is over a
        // blur, thinner towards the bottom, inside a pale rim.
        let top = f32::from(look.opacity) / 100.;
        let bottom = if look.frost { top * FROST_FADE } else { top };
        let tint = |alpha: f32| rgba((t.background << 8) | (alpha * 255.).round() as u32);
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_1()
            .border_color(if look.frost { rgba(FROST_RIM) } else { rgb(t.overlay).into() })
            .bg(linear_gradient(180., linear_color_stop(tint(top), 0.), linear_color_stop(tint(bottom), 1.)))
            .text_color(rgb(t.text))
            .child(bar)
            .child(div().id("pop-out-body").flex_1().min_h_0().overflow_y_scroll().child(body))
            .child(grip)
    }
}
