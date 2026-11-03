//! The strip at the top of the Snapshot section: when the Run started, who played it, how it stands,
//! a button for each day, which shows that day's last snapshot, and the button that pops the
//! Economy out over the game.

use bazaar_tracker::run::Outcome;
use gpui::{Context, Div, div, prelude::*, px, rgb};

use crate::ui::TrackerApp;
use crate::ui::widgets::{self, IconName, Tone};
use crate::tracking::run::header::{Header, header};

impl TrackerApp {
    /// The header of the selected Run, if one is selected.
    pub(in crate::ui) fn render_run_header(&self, cx: &mut Context<Self>) -> Option<Div> {
        let run = self.session.selected_run()?;
        let header = header(run, self.session.events(), self.session.is_live(&run.id), self.session.selected_day());
        let t = self.theme;
        let outcome_colour = |outcome: Outcome| if outcome == Outcome::Win { t.win } else { t.loss };
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
                        .items_start()
                        .gap_2()
                        .child(self.render_run_badges(&header))
                        .child(
                            widgets::icon_button(t, "pop-out", widgets::icon(IconName::PictureInPicture2, t.subtle))
                                .tooltip(widgets::hint(t, vec!["Pop the Economy and Shop odds out over the game".into()]))
                                .on_click(cx.listener(|this, _, _, cx| this.show_pop_out(cx))),
                        ),
                )
                .child(div().flex().flex_wrap().items_center().gap_1().children(header.timeline.iter().map(|node| {
                    let day = node.day;
                    let colour = match node.result {
                        Some(outcome) => outcome_colour(outcome),
                        None => t.highlight_med,
                    };
                    self.filter_button(format!("day-{day}"), format!("Day {day}"), node.selected)
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_day(day, cx)))
                        .child(div().size(px(6.)).rounded_full().bg(rgb(colour)))
                }))),
        )
    }

    /// Who played the Run and when, then how it stands as badges: rank, rating change, LIVE, the
    /// outcome and the record.
    fn render_run_badges(&self, header: &Header) -> Div {
        let t = self.theme;
        let outcome_colour = |outcome: Outcome| if outcome == Outcome::Win { t.win } else { t.loss };
        let change_colour = if header.rating_change.as_deref().is_some_and(|change| change.starts_with('-')) { t.loss } else { t.win };
        div()
            .flex()
            .flex_1()
            .flex_wrap()
            .items_center()
            .gap_1p5()
            .children(header.hero.clone().map(|hero| div().text_sm().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.heading)).child(hero)))
            .child(div().pr_1().text_xs().text_color(rgb(t.muted)).child(header.started.clone()))
            .children(header.rank.clone().map(|rank| widgets::badge(rank, t.subtle, Tone::Soft, t)))
            .children(header.rating_change.clone().map(|change| widgets::badge(change, change_colour, Tone::Soft, t)))
            .when(header.live, |row| row.child(widgets::live_badge(t)))
            .children(header.outcome.map(|outcome| {
                let word = if outcome == Outcome::Win { "Victory" } else { "Defeat" };
                widgets::badge(word, outcome_colour(outcome), Tone::Soft, t)
            }))
            .child(widgets::badge(header.record.clone(), t.subtle, Tone::Soft, t))
    }
}
