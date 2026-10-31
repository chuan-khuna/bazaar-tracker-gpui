//! The strip at the top of the Snapshot section: when the Run started, who played it, how it stands,
//! and a button for each day, which shows that day's last snapshot.

use bazaar_tracker::run::Outcome;
use gpui::{Context, Div, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::widgets;
use crate::tracking::header::header;

impl TrackerApp {
    /// The header of the selected Run, if one is selected.
    pub(super) fn render_run_header(&self, cx: &mut Context<Self>) -> Option<Div> {
        let run = self.session.selected_run()?;
        let header = header(run, self.session.events(), self.session.is_live(&run.id), self.session.selected_day());
        let t = self.theme;
        let outcome_colour = |outcome: Outcome| if outcome == Outcome::Win { t.win } else { t.loss };
        let change_colour = if header.rating_change.as_deref().is_some_and(|change| change.starts_with('-')) { t.loss } else { t.win };
        let overlay = t.overlay;
        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .gap_1p5()
                .px_3()
                .pt_2()
                .pb_2()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_x_3()
                        .gap_y_1()
                        .text_sm()
                        .child(div().text_color(rgb(t.subtle)).child(header.started.clone()))
                        .children(header.hero.clone().map(|hero| div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.heading)).child(hero)))
                        .children(header.rank.clone().map(|rank| div().text_color(rgb(t.subtle)).child(rank)))
                        .children(header.rating_change.clone().map(|change| div().text_color(rgb(change_colour)).child(change)))
                        .when(header.live, |row| row.child(widgets::chip(t, "LIVE", t.live)))
                        .children(header.outcome.map(|outcome| {
                            let word = if outcome == Outcome::Win { "Victory" } else { "Defeat" };
                            div().font_weight(gpui::FontWeight::SEMIBOLD).text_color(rgb(outcome_colour(outcome))).child(word)
                        }))
                        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_color(rgb(t.subtle)).child(header.record.clone())),
                )
                .child(div().flex().flex_wrap().items_center().gap_1().children(header.timeline.iter().map(|node| {
                    let day = node.day;
                    let colour = match node.result {
                        Some(outcome) => outcome_colour(outcome),
                        None => t.highlight_med,
                    };
                    div()
                        .id(gpui::SharedString::from(format!("day-{day}")))
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_1p5()
                        .py_0p5()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(if node.selected { t.accent } else { t.highlight_low }))
                        .cursor_pointer()
                        .text_size(px(11.))
                        .text_color(rgb(if node.selected { t.text } else { t.subtle }))
                        .when(node.selected, |button| button.bg(rgb(overlay)))
                        .hover(move |style| style.bg(rgb(overlay)))
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_day(day, cx)))
                        .child(format!("Day {day}"))
                        .child(div().size(px(7.)).rounded_full().bg(rgb(colour)))
                }))),
        )
    }
}
