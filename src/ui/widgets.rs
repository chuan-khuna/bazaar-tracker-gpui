//! Small pieces shared by the title bar, status bar, menu, command palette, pickers and panels.

use gpui::{AnyView, App, Context, Div, ElementId, Render, SharedString, Stateful, Svg, Window, div, prelude::*, px, rgb, rgba, svg};
use gpui_base::{Button, Scrollbar, ScrollbarHandle, ScrollbarMode};
use gpui_component::kbd::Kbd;

use super::Shortcut;
pub use gpui_kit_assets::IconName;

use crate::theme::Theme;

/// A floating box: the menu, the command palette, the pickers, the settings overlay.
pub fn panel(t: &Theme, id: &'static str) -> Stateful<Div> {
    div().id(id).occlude().flex().flex_col().rounded_lg().bg(rgb(t.surface)).border_1().border_color(rgb(t.highlight_med)).shadow_lg()
}

/// An island: a rounded panel floating on the window colour. It is `relative`, so a [`scrollbar`]
/// can be laid over it.
pub fn island(t: &Theme) -> Div {
    div().relative().flex().flex_col().min_w_0().h_full().rounded_xl().overflow_hidden().bg(rgb(t.surface))
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

/// How a [`badge`] is filled.
#[derive(Clone, Copy)]
pub enum Tone {
    /// Filled with its colour, in the background colour: for what must stand out, such as LIVE.
    Solid,
    /// Its colour on a faint wash of it: for a state, a time or a statistic.
    Soft,
}

/// The height of every badge, so that a row of them lines up.
pub const BADGE_HEIGHT: f32 = 18.;

/// The app's one kind of badge: a short label in `colour`, all the same height, font and corner.
pub fn badge(text: impl Into<SharedString>, colour: u32, tone: Tone, t: &Theme) -> Div {
    let (background, foreground) = match tone {
        Tone::Solid => (rgb(colour).into(), rgb(t.background)),
        Tone::Soft => (rgba((colour << 8) | 0x22), rgb(colour)),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(BADGE_HEIGHT))
        .px(px(6.))
        .rounded(px(3.))
        .bg(background)
        .text_color(foreground)
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(text.into())
}

/// The LIVE badge: the one mark for the game as it is now.
pub fn live_badge(t: &Theme) -> Div {
    badge("LIVE", t.live, Tone::Solid, t)
}

/// The text of a tooltip, in the theme's colours.
struct Hint {
    lines: Vec<SharedString>,
    surface: u32,
    border: u32,
    text: u32,
    subtle: u32,
}

impl Render for Hint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (first, rest) = self.lines.split_first().map_or((SharedString::default(), &[][..]), |(first, rest)| (first.clone(), rest));
        div()
            .m_3()
            .flex()
            .flex_col()
            .gap_0p5()
            .px_2()
            .py_1()
            .max_w(px(280.))
            .rounded_md()
            .bg(rgb(self.surface))
            .border_1()
            .border_color(rgb(self.border))
            .shadow_md()
            .text_xs()
            .text_color(rgb(self.subtle))
            .child(div().text_color(rgb(self.text)).child(first))
            .children(rest.iter().cloned())
    }
}

/// A tooltip showing `lines` (the first one brighter), for `.tooltip(...)`.
pub fn hint(t: &Theme, lines: Vec<String>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let (surface, border, text, subtle) = (t.surface, t.highlight_med, t.text, t.subtle);
    move |_, cx| cx.new(|_| Hint { lines: lines.iter().cloned().map(SharedString::from).collect(), surface, border, text, subtle }).into()
}

/// The app's scroll bar: a thin rounded thumb on a clear track, over the scroll area that `handle`
/// scrolls. It shows while the area is scrolled and fades out after, and widens and darkens under
/// the pointer and while dragged. It fills its parent, which must be `relative` and hold the area.
/// Vertical; `.axis(..)` changes that.
pub fn scrollbar<H: ScrollbarHandle + Clone>(t: &Theme, id: impl Into<ElementId>, handle: &H) -> Scrollbar {
    let colour = |hex: u32| gpui::Hsla::from(rgb(hex));
    let clear = gpui::transparent_black();
    Scrollbar::vertical(handle).id(id).mode(ScrollbarMode::Scrolling).styles(|styles| {
        styles
            .track(|track| track.bg(clear).border_color(clear))
            .track_hover(|track| track.bg(clear).border_color(clear))
            .track_active(|track| track.bg(clear).border_color(clear))
            .thumb(|thumb| thumb.bg(colour(t.highlight_med)).width(px(4.)).inset(px(3.)).radius(px(2.)))
            .thumb_hover(|thumb| thumb.bg(colour(t.muted)).width(px(6.)).inset(px(3.)).radius(px(3.)))
            .thumb_active(|thumb| thumb.bg(colour(t.subtle)).width(px(6.)).inset(px(3.)).radius(px(3.)))
    })
}

/// The same thin scroll bar, along the bottom of a scroll area that scrolls sideways.
pub fn hscrollbar<H: ScrollbarHandle + Clone>(t: &Theme, id: impl Into<ElementId>, handle: &H) -> Scrollbar {
    let colour = |hex: u32| gpui::Hsla::from(rgb(hex));
    let clear = gpui::transparent_black();
    Scrollbar::horizontal(handle).id(id).mode(ScrollbarMode::Scrolling).styles(|styles| {
        styles
            .track(|track| track.bg(clear).border_color(clear))
            .track_hover(|track| track.bg(clear).border_color(clear))
            .track_active(|track| track.bg(clear).border_color(clear))
            .thumb(|thumb| thumb.bg(colour(t.highlight_med)).width(px(4.)).inset(px(3.)).radius(px(2.)))
            .thumb_hover(|thumb| thumb.bg(colour(t.muted)).width(px(6.)).inset(px(3.)).radius(px(3.)))
            .thumb_active(|thumb| thumb.bg(colour(t.subtle)).width(px(6.)).inset(px(3.)).radius(px(3.)))
    })
}

/// A section's title: `SNAPSHOT`, `ECONOMY`, `STASH`. Badges and counts go beside it.
pub fn section_title(t: &Theme, text: impl Into<SharedString>) -> Div {
    div().flex_none().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(text.into())
}

/// A section's header row: the title on the left, then `left` beside it, `right` against the
/// right edge, all on one line of badge height.
pub fn section_header(left: impl IntoIterator<Item = Div>, right: impl IntoIterator<Item = Div>) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1p5()
        .min_h(px(BADGE_HEIGHT))
        .children(left)
        .child(div().flex().items_center().gap_1p5().ml_auto().children(right))
}
