//! The list of the selected Run's Events, under the board. Clicking an Event moves the board to its
//! moment and fills the detail panel; clicking it again lets go.

use gpui::{Context, Div, SharedString, Stateful, div, prelude::*, px, rgb};

use super::TrackerApp;
use crate::tracking::events::{EventRow, Tone, rows};

impl TrackerApp {
    pub(super) fn render_events(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let rows: Vec<_> = rows(self.session.events(), self.session.selected_event(), &self.names).into_iter().map(|row| self.render_event_row(row, cx)).collect();
        div()
            .id("events")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .px_1p5()
            .pb_1p5()
            .border_t_1()
            .border_color(rgb(t.overlay))
            .overflow_y_scroll()
            .track_scroll(&self.events_scroll)
            .children(rows)
    }

    fn render_event_row(&self, row: EventRow, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let overlay = t.overlay;
        let description = match row.tone {
            Tone::Plain => t.subtle,
            Tone::Win => t.win,
            Tone::Loss => t.loss,
        };
        let id = row.id.clone();
        div()
            .id(SharedString::from(format!("event-{}", row.id)))
            .flex()
            .flex_none()
            .items_center()
            .gap_3()
            .h(px(24.))
            .px_2()
            .rounded_md()
            .text_xs()
            .cursor_pointer()
            .when(row.selected, |line| line.bg(rgb(overlay)))
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_event(&id, cx)))
            .child(div().flex_none().w(px(44.)).text_color(rgb(t.muted)).child(row.time))
            .child(div().flex_none().w(px(44.)).text_color(rgb(t.muted)).child(row.hour))
            .child(div().flex_none().w(px(96.)).when(row.sub, |kind| kind.pl_3()).text_color(rgb(t.text)).child(if row.sub { format!("↳ {}", row.kind) } else { row.kind.to_owned() }))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_color(rgb(description)).child(row.description))
    }
}
