//! The list of the selected Run's Events, under the board. Clicking an Event moves the board to its
//! moment and fills the detail panel; clicking it again lets go. Above it, a row of day buttons and
//! a row of hour buttons narrow the list down.

use gpui::{Context, Div, SharedString, Stateful, div, prelude::*, px, rgb};

use super::TrackerApp;
use crate::tracking::events::{EventRow, Tone, days, hours, rows};

impl TrackerApp {
    pub(super) fn render_events(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        div().flex().flex_col().flex_none().border_t_1().border_color(rgb(t.overlay)).child(self.render_event_filter(cx)).child(self.render_event_list(cx))
    }

    /// `All D1 D2 ... Dn` and `All H1 ... H6`: the chosen one is outlined.
    fn render_event_filter(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let filter = self.session.filter();
        let events = self.session.events();
        let day_buttons = std::iter::once(None).chain(days(events).into_iter().map(Some)).map(|day| {
            let label = day.map_or_else(|| "All".to_owned(), |day| format!("D{day}"));
            self.filter_button(format!("filter-day-{label}"), label, filter.day == day).on_click(cx.listener(move |this, _, _, cx| this.filter_day(day, cx)))
        });
        let hour_buttons = std::iter::once(None).chain(hours(events).into_iter().map(Some)).map(|hour| {
            let label = hour.map_or_else(|| "All".to_owned(), |hour| format!("H{hour}"));
            self.filter_button(format!("filter-hour-{label}"), label, filter.hour == hour).on_click(cx.listener(move |this, _, _, cx| this.filter_hour(hour, cx)))
        });
        let line = || div().flex().flex_wrap().items_center().gap_1();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap_1()
            .px_3()
            .pt_2()
            .pb_1()
            .child(line().child(div().w(px(36.)).text_size(px(11.)).text_color(rgb(t.muted)).child("Day")).children(day_buttons))
            .child(line().child(div().w(px(36.)).text_size(px(11.)).text_color(rgb(t.muted)).child("Hour")).children(hour_buttons))
    }

    fn filter_button(&self, id: String, label: String, chosen: bool) -> Stateful<Div> {
        let t = self.theme;
        let overlay = t.overlay;
        div()
            .id(SharedString::from(id))
            .px_1p5()
            .py_0p5()
            .rounded_md()
            .border_1()
            .border_color(rgb(if chosen { t.accent } else { t.highlight_low }))
            .cursor_pointer()
            .text_size(px(11.))
            .text_color(rgb(if chosen { t.text } else { t.subtle }))
            .when(chosen, |button| button.bg(rgb(overlay)))
            .hover(move |style| style.bg(rgb(overlay)))
            .child(label)
    }

    fn render_event_list(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let rows: Vec<_> = rows(self.session.events(), self.session.selected_event(), self.session.filter(), &self.names).into_iter().map(|row| self.render_event_row(row, cx)).collect();
        div()
            .id("events")
            .flex()
            .flex_col()
            .flex_none()
            .h(px(220.))
            .px_1p5()
            .pb_1p5()
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
