//! The strip at the top of the detail panel: when the Run started, who played it and how it stands.

use bazaar_tracker::run::Outcome;
use gpui::{Div, div, prelude::*, rgb};

use crate::ui::TrackerApp;
use crate::ui::widgets::{self, Tone};
use crate::tracking::run::header::{Header, header};

impl TrackerApp {
    /// The header of the selected Run, if one is selected.
    pub(in crate::ui) fn render_run_header(&self) -> Option<Div> {
        let run = self.session.selected_run()?;
        let header = header(run, self.session.events(), self.session.is_live(&run.id), self.session.selected_day());
        Some(div().flex().flex_none().px_3().py_2().child(self.render_run_badges(&header)))
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
