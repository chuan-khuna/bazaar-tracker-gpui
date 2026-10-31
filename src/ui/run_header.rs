//! The strip at the top of the main island: when the Run started, who played it, how it stands and
//! how each day went.

use bazaar_tracker::run::Outcome;
use gpui::{Div, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::widgets;
use crate::theme::Theme;
use crate::tracking::header::{Header, header};

impl TrackerApp {
    /// The header of the selected Run, if one is selected.
    pub(super) fn render_run_header(&self) -> Option<Div> {
        let run = self.session.selected_run()?;
        let header = header(run, self.session.events(), self.session.is_live(&run.id));
        Some(draw_header(self.theme, &header))
    }
}

fn draw_header(t: &Theme, header: &Header) -> Div {
    let outcome_colour = |outcome: Outcome| if outcome == Outcome::Win { t.win } else { t.loss };
    let change_colour = if header.rating_change.as_deref().is_some_and(|change| change.starts_with('-')) { t.loss } else { t.win };
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap_1p5()
        .px_3()
        .pt_3()
        .pb_2()
        .border_b_1()
        .border_color(rgb(t.overlay))
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
                })),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x_3()
                .gap_y_1()
                .text_sm()
                .children(header.day.clone().map(|day| div().text_color(rgb(t.text)).child(day)))
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_color(rgb(t.subtle)).child(header.record.clone()))
                .child(div().flex().flex_wrap().items_center().gap_1p5().children(header.timeline.iter().map(|node| {
                    let colour = match node.result {
                        Some(outcome) => outcome_colour(outcome),
                        None => t.highlight_med,
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap_0p5()
                        .child(div().text_size(px(10.)).text_color(rgb(t.muted)).child(format!("D{}", node.day)))
                        .child(div().size(px(8.)).rounded_full().bg(rgb(colour)))
                }))),
        )
}
