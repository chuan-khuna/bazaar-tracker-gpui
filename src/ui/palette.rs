//! The command palette, in the middle of the window: a search field, then the matching commands.
//! Its state and keys are in `overlays/commands.rs`.

use gpui::{Context, SharedString, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::widgets::{panel, query_field};
use crate::overlays::commands::{Command, CommandPalette};

impl TrackerApp {
    pub(super) fn render_palette(&self, palette: &CommandPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let matches = palette.matches();
        let mut body = div().flex().flex_col().p_1();
        for (index, command) in matches.iter().copied().enumerate() {
            body = body.child(
                self.command_row(command, SharedString::from(format!("palette-{index}")))
                    .when(index == palette.selected, |row| row.bg(rgb(t.overlay)))
                    .on_click(cx.listener(move |this, _, window, cx| this.choose_in_palette(command, window, cx))),
            );
        }
        if matches.is_empty() {
            body = body.child(div().px_2().py_1().text_sm().text_color(rgb(t.muted)).child("No matching commands"));
        }

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(panel(t, "command-palette").w(px(360.)).child(query_field(t, &palette.query, "Type a command…")).child(body))
    }

    fn choose_in_palette(&mut self, command: Command, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.dismiss(cx);
        self.run(command, window, cx);
    }
}
