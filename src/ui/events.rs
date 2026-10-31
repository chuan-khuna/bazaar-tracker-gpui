//! The list of the selected Run's Events, under the board. Clicking an Event moves the board to its
//! moment and fills the detail panel; clicking it again lets go. Above it, a row of day buttons and
//! a row of hour buttons narrow the list down.

use std::ops::Range;

use gpui::{Context, Div, SharedString, Stateful, UniformList, div, prelude::*, px, rgb, uniform_list};

use super::TrackerApp;
use crate::tracking::events::{EventRow, Tone, days, hours, rows};

impl TrackerApp {
    pub(super) fn render_events(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        div().flex().flex_col().flex_none().gap_1().border_t_1().border_color(rgb(t.overlay)).child(self.render_event_filter(cx)).child(self.render_event_list(cx))
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

    /// A table: a header that stays put, then the rows, of which only those in view are drawn
    /// (every row is the same height), with a scroll bar over them.
    fn render_event_list(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let rows = rows(self.session.events(), self.session.selected_event(), self.session.filter(), &self.names);
        let body: UniformList = uniform_list("events", rows.len(), cx.processor(move |this, range: Range<usize>, _, cx| range.map(|index| this.render_event_row(index, rows[index].clone(), cx)).collect::<Vec<_>>()))
            .size_full()
            .track_scroll(&self.events_scroll);
        let header = |text: &'static str| div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(text);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .mx_3()
            .mb_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(t.overlay))
            .overflow_hidden()
            .child(columns(div().h(px(ROW_HEIGHT)).bg(rgb(t.highlight_low)).border_b_1().border_color(rgb(t.overlay)), [header("TIME"), header("DAY"), header("HOUR"), header("EVENT"), header("DETAIL")]))
            .child(div().relative().h(px(220.)).child(body).child(super::widgets::scrollbar(t, "events-scrollbar", &self.events_scroll)))
    }

    fn render_event_row(&self, index: usize, row: EventRow, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let overlay = t.overlay;
        let description = match row.tone {
            Tone::Plain => t.subtle,
            Tone::Win => t.win,
            Tone::Loss => t.loss,
        };
        // The day and hour stand out on the first row of an hour, and are faint after it.
        let when = if row.moved { t.text } else { t.muted };
        let id = row.id.clone();
        // A choice or a sale is a tag, indented under the Event it came from.
        let kind = if row.sub {
            let colour = if row.kind == "Sold" { t.gold } else { t.accent };
            div().flex().items_center().gap_1().pl_3().child(div().text_color(rgb(t.muted)).child("↳")).child(super::widgets::tag(row.kind, colour))
        } else {
            div().text_color(rgb(t.text)).child(row.kind)
        };
        let line = div()
            .id(SharedString::from(format!("event-{}", row.id)))
            .h(px(ROW_HEIGHT))
            .cursor_pointer()
            .when(row.moved && index > 0, |line| line.border_t_1().border_color(rgb(t.overlay)))
            .when(index % 2 == 1, |line| line.bg(rgb(t.highlight_low)))
            .when(row.selected, |line| line.bg(rgb(overlay)))
            .hover(move |style| style.bg(rgb(overlay)))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_event(&id, cx)));
        columns(
            line,
            [
                div().text_color(rgb(t.muted)).child(row.time),
                div().text_color(rgb(when)).child(row.day),
                div().text_color(rgb(when)).child(row.hour),
                kind,
                div().overflow_hidden().whitespace_nowrap().text_color(rgb(description)).child(row.description),
            ],
        )
    }
}

/// The height of the header and of every row.
const ROW_HEIGHT: f32 = 24.;

/// The widths of the time, day, hour and event columns; the detail takes the rest.
const COLUMNS: [f32; 4] = [44., 36., 36., 110.];

/// Lay the five cells of a header or a row out in the table's columns.
fn columns<E: ParentElement + Styled>(line: E, [time, day, hour, kind, detail]: [Div; 5]) -> E {
    line.flex()
        .flex_none()
        .items_center()
        .gap_3()
        .px_2()
        .text_xs()
        .child(time.flex_none().w(px(COLUMNS[0])))
        .child(day.flex_none().w(px(COLUMNS[1])))
        .child(hour.flex_none().w(px(COLUMNS[2])))
        .child(kind.flex_none().w(px(COLUMNS[3])))
        .child(detail.flex_1().min_w_0())
}
