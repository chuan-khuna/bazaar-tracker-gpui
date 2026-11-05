//! The ☰ button in the title bar and the menu that hangs from it. Its rows are `Command`s, drawn
//! by `command_row`, which the command palette uses too.

use gpui::{Anchor, AnyElement, Context, Div, ElementId, SharedString, Stateful, anchored, deferred, div, prelude::*, px, rgb};

use crate::ui::widgets::{self, menu_item, panel, separator};
use crate::ui::{DETAIL_SHORTCUT, LOG_SHORTCUT, PALETTE_SHORTCUT, QUIT_SHORTCUT, SIDEBAR_SHORTCUT, Shortcut, THEME_PICKER_SHORTCUT, TrackerApp};
use crate::overlays::Overlay;
use crate::overlays::commands::{Command, Menu};
use crate::theme::Theme;

/// The shortcut that runs `command`, if it has one.
fn shortcut_of(command: Command) -> Option<&'static Shortcut> {
    match command {
        Command::ToggleSidebar => Some(&SIDEBAR_SHORTCUT),
        Command::ToggleDetail => Some(&DETAIL_SHORTCUT),
        Command::ToggleLog => Some(&LOG_SHORTCUT),
        Command::ChangeTheme => Some(&THEME_PICKER_SHORTCUT),
        Command::Quit => Some(&QUIT_SHORTCUT),
        Command::Settings | Command::ExtractArt | Command::ClearData | Command::ToggleRunDuration | Command::ResetLayout => None,
    }
}

impl TrackerApp {
    /// Whether `command` is a toggle, and if so whether it is on.
    fn toggled(&self, command: Command) -> Option<bool> {
        let settings = self.settings.get();
        match command {
            Command::ToggleSidebar => Some(settings.window.sidebar_open),
            Command::ToggleDetail => Some(self.detail_open),
            Command::ToggleLog => Some(settings.window.log_open),
            Command::ToggleRunDuration => Some(settings.show_run_duration),
            _ => None,
        }
    }

    /// The dim value at the right of a command's row, if it has one.
    fn value_of(&self, command: Command) -> Option<SharedString> {
        match command {
            Command::ChangeTheme => Some(self.theme.name.into()),
            _ => None,
        }
    }

    /// A command as a row: its label, and on the right its current value, its shortcut and an
    /// on-dot (for a toggle that is on).
    pub(in crate::ui) fn command_row(&self, command: Command, id: impl Into<ElementId>) -> Stateful<Div> {
        let t = self.theme;
        let right = div()
            .flex()
            .items_center()
            .gap_2()
            .children(self.value_of(command).map(|value| div().text_xs().text_color(rgb(t.muted)).child(value)))
            .children(shortcut_of(command).map(|shortcut| widgets::shortcut(t, shortcut)))
            .children(self.toggled(command).map(|on| on_dot(t, on)));
        menu_item(t, id, command.label(), right)
    }

    /// The ☰ button, with the menu hanging from its bottom-right corner while it is open.
    pub(in crate::ui) fn render_menu_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let overlay = self.theme.overlay;
        let menu = match &self.overlay {
            Some(Overlay::Menu(menu)) => Some(menu),
            _ => None,
        };
        let bars = div().flex().flex_col().gap(px(3.)).children((0..3).map(|_| div().w(px(14.)).h(px(2.)).rounded_sm().bg(rgb(self.theme.subtle))));

        div()
            .relative()
            .child(
                div()
                    .id("menu")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(26.))
                    .rounded_md()
                    .cursor_pointer()
                    .occlude()
                    .when(menu.is_some(), |button| button.bg(rgb(overlay)))
                    .hover(move |style| style.bg(rgb(overlay)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        window.focus(&this.focus_handle, cx);
                        this.toggle_menu(cx);
                    }))
                    .child(bars),
            )
            .when_some(menu, |container, menu| {
                container.child(
                    div().absolute().top_full().right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window().child(self.render_menu(menu, cx)))),
                )
            })
            .into_any_element()
    }

    fn render_menu(&self, menu: &Menu, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let commands = menu.commands();
        let mut list = panel(t, "menu-list").mt_1().w(px(340.)).p_1();
        for (index, command) in commands.iter().copied().enumerate() {
            if index > 0 && commands[index - 1].group() != command.group() {
                list = list.child(separator(t));
            }
            list = list.child(self.command_row(command, SharedString::from(format!("menu-{index}"))).on_click(cx.listener(move |this, _, window, cx| this.run(command, window, cx))));
        }
        list.child(separator(t)).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(30.))
                .px_2()
                .text_sm()
                .text_color(rgb(t.muted))
                .child("Command palette")
                .child(widgets::shortcut(t, &PALETTE_SHORTCUT)),
        )
    }
}

/// The mark for a toggle that is on: a dot. Off, the space stays empty so the rows line up.
fn on_dot(t: &Theme, on: bool) -> impl IntoElement {
    div().w(px(16.)).flex().justify_center().when(on, |slot| slot.child(div().size(px(6.)).rounded_full().bg(rgb(t.success))))
}
