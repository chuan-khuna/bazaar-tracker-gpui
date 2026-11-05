//! A flat title bar drawn by the app, holding the title and the ☰ button. On macOS the system title bar is made
//! transparent and the traffic lights sit on top of this bar; macOS still drags the
//! window by it. On Windows the bar draws its own caption buttons and tells the
//! system which areas drag, minimise, maximise and close. Linux keeps its system title bar.

use gpui::{
    AnyElement, ClickEvent, Pixels, Point, TitlebarOptions, Window,
    WindowControlArea, div, point, prelude::*, px, rgb,
};

use crate::theme::Theme;

const HEIGHT: Pixels = px(34.);
/// Room on the left for the macOS traffic lights.
const TRAFFIC_LIGHTS_WIDTH: Pixels = px(88.);
const TRAFFIC_LIGHTS_POSITION: Point<Pixels> = point(px(12.), px(11.));

pub fn options() -> TitlebarOptions {
    TitlebarOptions {
        title: Some("Bazaar Tracker".into()),
        appears_transparent: true,
        traffic_light_position: Some(TRAFFIC_LIGHTS_POSITION),
    }
}

/// The bar holds only `trailing` on the right (the hamburger); the rest of it drags the window.
/// `trailing` should `occlude()` so clicks on it don't drag the window.
pub fn render(theme: &Theme, trailing: AnyElement, window: &Window) -> impl IntoElement {
    let left_padding = if cfg!(target_os = "macos") { TRAFFIC_LIGHTS_WIDTH } else { px(12.) };


    div()
        .id("title-bar")
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .w_full()
        .h(HEIGHT)
        .pl(left_padding)
        .bg(rgb(theme.background))
        .window_control_area(WindowControlArea::Drag)
        .on_click(|event: &ClickEvent, window, _| {
            if event.click_count() == 2 {
                if cfg!(target_os = "macos") { window.titlebar_double_click() } else { window.zoom_window() }
            }
        })
        .child(div().text_xs().text_color(rgb(theme.subtle)).child("Bazaar Tracker"))
        .child(
            div()
                .flex()
                .items_center()
                .h_full()
                .gap_1()
                .when(!cfg!(target_os = "windows"), |right| right.pr_2())
                .child(trailing)
                .when(cfg!(target_os = "windows"), |right| right.child(caption_buttons(theme, window))),
        )
}

/// Minimise, maximise (or restore) and close, drawn with the Segoe icon fonts like Zed does.
fn caption_buttons(theme: &Theme, window: &Window) -> AnyElement {
    let maximise = if window.is_maximized() { "\u{e923}" } else { "\u{e922}" };
    div()
        .flex()
        .h_full()
        .child(caption_button(theme, "minimise", "\u{e921}", WindowControlArea::Min, theme.highlight_med))
        .child(caption_button(theme, "maximise", maximise, WindowControlArea::Max, theme.highlight_med))
        .child(caption_button(theme, "close", "\u{e8bb}", WindowControlArea::Close, theme.error))
        .into_any_element()
}

fn caption_button(theme: &Theme, id: &'static str, icon: &'static str, area: WindowControlArea, hover: u32) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(46.))
        .h_full()
        .font_family("Segoe Fluent Icons")
        .text_size(px(10.))
        .text_color(rgb(theme.subtle))
        // Without this the bar's Drag area sits under the cursor too, and gpui reports
        // the first area it registered (the bar's), so the click drags instead.
        .occlude()
        .window_control_area(area)
        .hover({
            let text = theme.text;
            move |style| style.bg(rgb(hover)).text_color(rgb(text))
        })
        .child(icon)
}
