//! The detail panel on the right. It shows what is specific to the selected Event: the options that
//! were offered, or the boards of a combat.

use gpui::{Div, Pixels, div, prelude::*, px, rgb};

use super::TrackerApp;
use super::widgets::{empty_state, heading, island};
use crate::theme::Theme;
use crate::tracking::board::Tier;
use crate::tracking::detail::{CardSection, CardView, detail};

/// The detail panel's width.
pub const WIDTH: Pixels = px(300.);

fn tier_colour(t: &Theme, tier: Option<Tier>) -> u32 {
    match tier {
        Some(Tier::Bronze) => t.bronze,
        Some(Tier::Silver) => t.silver,
        Some(Tier::Gold) => t.gold,
        Some(Tier::Diamond) => t.diamond,
        Some(Tier::Legendary) => t.legendary,
        None => t.highlight_med,
    }
}

impl TrackerApp {
    pub(super) fn render_detail(&self) -> impl IntoElement {
        let t = self.theme;
        let panel = island(t).flex_none().w(WIDTH);
        let Some(selected) = self.session.selected_event() else {
            return panel.child(heading(t, "DETAIL")).child(empty_state(t, "Select an event to see its details."));
        };
        let view = detail(self.session.events(), selected, &self.names);
        panel.child(div().flex_none().px_3().pt_3().pb_1().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(view.title.clone())).child(
            div()
                .id("detail-body")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap_2()
                .px_3()
                .pb_3()
                .overflow_y_scroll()
                .children(view.banner.clone().map(|banner| div().text_sm().text_color(rgb(t.text)).child(banner)))
                .when(!view.facts.is_empty(), |body| {
                    body.child(div().flex().flex_col().gap_0p5().text_xs().children(view.facts.iter().map(|(label, value)| {
                        div().flex().gap_2().child(div().flex_none().w(px(104.)).text_color(rgb(t.muted)).child(*label)).child(div().text_color(rgb(t.text)).child(value.clone()))
                    })))
                })
                .children(view.sections.iter().map(|section| self.render_card_section(section))),
        )
    }

    fn render_card_section(&self, section: &CardSection) -> Div {
        let t = self.theme;
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(section.title))
            .when(section.cards.is_empty(), |column| column.child(div().text_xs().text_color(rgb(t.muted)).child("none")))
            .children(section.cards.iter().map(|card| self.render_card(card)))
    }

    pub(super) fn render_card(&self, card: &CardView) -> Div {
        let t = self.theme;
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_md()
            .border_l_4()
            .border_color(rgb(tier_colour(t, card.tier)))
            .bg(rgb(if card.picked { t.overlay } else { t.highlight_low }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().text_color(rgb(t.text)).child(card.name.clone()))
                    .child(div().text_xs().text_color(rgb(t.subtle)).child(card.meta.clone())),
            )
            .when(card.picked, |row| row.child(div().text_sm().text_color(rgb(t.success)).child("✓")))
    }
}
