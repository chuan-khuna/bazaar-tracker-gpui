//! Small pieces shared by the title bar, status bar, menu, command palette, pickers and panels.

use gpui::{Div, ElementId, Stateful, Svg, div, prelude::*, px, rgb, svg};
use gpui_base::Button;
use gpui_component::kbd::Kbd;

use super::Shortcut;
pub use gpui_kit_assets::IconName;

use crate::theme::Theme;

/// A floating box: the menu, the command palette, the pickers, the settings overlay.
pub fn panel(t: &Theme, id: &'static str) -> Stateful<Div> {
    div().id(id).occlude().flex().flex_col().rounded_lg().bg(rgb(t.surface)).border_1().border_color(rgb(t.highlight_med)).shadow_lg()
}

/// An island: a rounded panel floating on the window colour.
pub fn island(t: &Theme) -> Div {
    div().flex().flex_col().min_w_0().h_full().rounded_xl().overflow_hidden().bg(rgb(t.surface))
}

/// The heading along the top of an island.
pub fn heading(t: &Theme, text: &'static str) -> Div {
    div().flex_none().px_3().pt_3().pb_1().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(text)
}

/// The text of an empty state, centred in the rest of its island.
pub fn empty_state(t: &Theme, text: &'static str) -> Div {
    div().flex_1().flex().items_center().justify_center().p_4().text_sm().text_center().text_color(rgb(t.muted)).child(text)
}

/// The text field along the top of the palette and the picker: what is typed (or a dim placeholder), then a caret.
pub fn query_field(t: &Theme, text: &str, placeholder: &str) -> Div {
    let text = if text.is_empty() { div().text_color(rgb(t.muted)).child(placeholder.to_string()) } else { div().text_color(rgb(t.text)).child(text.to_string()) };
    div().flex().items_center().h(px(36.)).px_3().border_b_1().border_color(rgb(t.overlay)).text_sm().child(text).child(caret(t))
}

pub fn caret(t: &Theme) -> Div {
    div().w(px(1.5)).h(px(14.)).ml(px(1.)).bg(rgb(t.accent))
}

/// A thin line between groups of menu rows.
pub fn separator(t: &Theme) -> Div {
    div().my_1().h(px(1.)).bg(rgb(t.overlay))
}

/// A row of a menu or list: the label on the left, and `right` (a detail, an on-dot, a shortcut) on the right.
pub fn menu_item(t: &Theme, id: impl Into<ElementId>, label: &'static str, right: impl IntoElement) -> Stateful<Div> {
    let overlay = t.overlay;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .h(px(30.))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(overlay)))
        .child(div().text_sm().text_color(rgb(t.text)).child(label))
        .child(right)
}

/// A shortcut as the kit's `Kbd` keys, one for each key press of a chord, in the theme's colours.
pub fn shortcut(t: &Theme, shortcut: &Shortcut) -> Div {
    div().flex().items_center().gap_1().children(shortcut.keystrokes().into_iter().map(|stroke| {
        Kbd::new(stroke).text_color(rgb(t.subtle)).bg(rgb(t.overlay)).border_1().border_color(rgb(t.highlight_med)).rounded_sm()
    }))
}

/// A small square button around an icon. It does not take keyboard focus, so typing keeps going to the window.
pub fn icon_button(t: &Theme, id: impl Into<ElementId>, icon: impl IntoElement) -> Button {
    let overlay = t.overlay;
    Button::new(id)
        .focusable(false)
        .tab_stop(false)
        .flex()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded_md()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(overlay)))
        .child(icon)
}

/// One of the kit's icons, in `colour`.
pub fn icon(name: IconName, colour: u32) -> Svg {
    svg().path(name.path()).size(px(14.)).flex_none().text_color(rgb(colour))
}

/// A small capsule of text, such as LIVE.
pub fn chip(t: &Theme, text: &'static str, colour: u32) -> Div {
    div().flex_none().px_1p5().rounded_full().bg(rgb(colour)).text_color(rgb(t.background)).text_size(px(10.)).font_weight(gpui::FontWeight::BOLD).child(text)
}
