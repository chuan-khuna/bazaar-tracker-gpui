//! The pop-out: the Economy, the Shop odds and the Item odds, one under another, in a small window
//! over a blur of what is behind it, which stays above every other window, the game's too. Its top
//! strip shows the game's day and hour and drags it, its ✕ shuts it, and the grip in its bottom
//! right corner resizes it. While the main window is in front the pop-out is closed; once the main
//! window is left it opens again where it was, without taking the keyboard from the game.

use gpui::{
    App, Bounds, Context, CursorStyle, Div, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Size, Window, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowHandle, WindowKind, WindowOptions, div, point, prelude::*, px, rgb, rgba, size,
};

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
/// How opaque its background is over the blur, out of 0xFF.
const ALPHA: u32 = 0xB0;

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

    /// The ↗ button: opens the pop-out in front, where it last was.
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
            let opened = cx.open_window(options(bounds, focus), |window, cx| {
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

fn options(bounds: Bounds<Pixels>, focus: bool) -> WindowOptions {
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
        window_background: WindowBackgroundAppearance::Blurred,
        ..Default::default()
    }
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

    /// The Economy, the Shop odds and the Item odds, one under another, as wide as the pop-out.
    fn render_pop_out_body(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let empty = |title: &'static str, text: &'static str| {
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .px_3()
                .py_2()
                .child(widgets::section_header([widgets::section_title(t, title)], []))
                .child(div().text_xs().text_color(rgb(t.muted)).child(text))
        };
        let attached = self.status.connection == Connection::Tracking;
        let item_odds = match self.live.view(attached, self.session.live().is_some(), &self.names) {
            Live::View(view) => self.render_choice_odds(&view, cx),
            Live::Placeholder(_) => None,
        };
        let rule = || div().flex_none().h(px(1.)).mx_3().bg(rgb(t.overlay));
        div()
            .flex()
            .flex_col()
            .w_full()
            // The Economy grows sideways in a row of its own.
            .child(div().flex().w_full().child(self.render_economy().unwrap_or_else(|| empty("ECONOMY", "No gold to chart yet."))))
            .child(rule())
            .child(self.upcoming_odds_column().w_full())
            .child(rule())
            .child(match item_odds {
                Some(odds) => odds.px_3().py_2(),
                None => empty("ITEM ODDS", "Shown at an encounter choice."),
            })
    }
}

/// The pop-out window's view: it draws from the main window's [`TrackerApp`] and redraws with it.
pub(in crate::ui) struct PopOut {
    app: Entity<TrackerApp>,
    /// While the grip is held: where the pointer was pressed, and the window's size then. The
    /// window grows from its top left corner, which stays put, so the pointer's place in the
    /// window measures the drag.
    resizing: Option<(Point<Pixels>, Size<Pixels>)>,
}

impl PopOut {
    fn new(app: Entity<TrackerApp>, cx: &mut Context<Self>) -> Self {
        cx.observe(&app, |_, _, cx| cx.notify()).detach();
        Self { app, resizing: None }
    }

    fn drag_grip(&mut self, event: &MouseMoveEvent, window: &mut Window) {
        let Some((start, from)) = self.resizing else { return };
        // The button came up outside the window.
        if event.pressed_button != Some(MouseButton::Left) {
            self.resizing = None;
            return;
        }
        let width = (from.width + event.position.x - start.x).max(px(MIN_SIZE.0));
        let height = (from.height + event.position.y - start.y).max(px(MIN_SIZE.1));
        window.resize(size(width, height));
    }
}

impl Render for PopOut {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = cx.listener(|this, _, window, cx| {
            this.app.update(cx, |app, _| app.pop_out = PopOutState::Off);
            window.remove_window();
        });
        let (t, time, body) = self.app.update(cx, |app, cx| (app.theme, app.pop_out_time(), app.render_pop_out_body(cx)));
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, _| this.resizing = Some((event.position, window.viewport_size()))),
            );
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_1()
            .border_color(rgb(t.overlay))
            .bg(rgba((t.background << 8) | ALPHA))
            .text_color(rgb(t.text))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, _| this.drag_grip(event, window)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.resizing = None))
            .child(bar)
            .child(div().id("pop-out-body").flex_1().min_h_0().overflow_y_scroll().child(body))
            .child(grip)
    }
}
