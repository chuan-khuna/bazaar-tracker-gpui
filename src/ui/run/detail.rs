//! The detail panel on the right, and the strip on its left that resizes it, as the sidebar's does.
//! It shows the selected Run's Snapshot at the chosen moment, then what is specific to the selected
//! Event: the options that were offered, or the boards of a combat.

use gpui::{Context, Div, MouseButton, Pixels, div, prelude::*, px, rgb};

use crate::ui::TrackerApp;
use crate::ui::run::board::{MarkSide, Section};
use crate::ui::frame::sidebar::GAP;
use crate::ui::widgets::{self, empty_state, heading, island};
use crate::shell::settings::clamp_detail_width;
use crate::theme::Theme;
use crate::tracking::run::board::{self, Tier};
use crate::tracking::run::detail::{BoardBox, CardSection, CardView, detail};

/// The row of islands has this much padding on its right.
const ROW_PADDING: f32 = 8.;

impl TrackerApp {
    /// The strip between the main island and the detail island: drag it to change the detail's width.
    pub(in crate::ui) fn render_detail_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .id("detail-handle")
            .flex_none()
            .w(GAP)
            .h_full()
            .cursor_col_resize()
            .when(self.resizing_detail, |handle| handle.child(div().mx_auto().w(px(2.)).h_full().bg(rgb(t.accent))))
            .hover(move |style| style.bg(rgb(t.overlay)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.resizing_detail = true;
                    cx.notify();
                }),
            )
    }

    /// Follow the pointer while the handle is held. The detail island ends at the row's right
    /// padding, or at the log panel when that is open. The width is saved when the button is released.
    pub(in crate::ui) fn drag_detail(&mut self, x: Pixels, window_width: Pixels, cx: &mut Context<Self>) {
        if self.resizing_detail {
            let log = if self.settings.get().window.log_open { self.log_width as f32 + f32::from(GAP) } else { 0. };
            let right = f32::from(window_width) - ROW_PADDING - log;
            let width = (right - f32::from(x) - f32::from(GAP) / 2.).round().max(0.) as u32;
            self.detail_width = clamp_detail_width(width);
            cx.notify();
        }
    }

    pub(in crate::ui) fn end_detail_drag(&mut self, cx: &mut Context<Self>) {
        if self.resizing_detail {
            self.resizing_detail = false;
            let width = self.detail_width;
            // A failed save is logged; the width is still in use.
            if let Err(error) = self.settings.update(|settings| settings.window.set_detail_width(width)) {
                tracing::error!("Could not save settings: {error}");
            }
            cx.notify();
        }
    }
}

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
    /// The detail island: the selected Run's Snapshot at the chosen moment (it folds away), then
    /// what is specific to the selected Event. The two scroll together.
    pub(in crate::ui) fn render_detail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let snapshot = self.session.selected_run().map(|run| {
            let view = board::view(self.session.events(), self.session.choice(), self.session.is_live(&run.id), &self.names);
            div()
                .flex()
                .flex_col()
                .flex_none()
                .border_b_1()
                .border_color(rgb(t.overlay))
                .child(self.render_board_section(&view, Section::Snapshot, Vec::new(), cx))
        });
        let event = match self.session.selected_event() {
            None => div().flex().flex_col().flex_none().child(heading(t, "DETAIL")).child(empty_state(t, "Select an event to see its details.")),
            Some(selected) => {
                let view = detail(self.session.events(), selected, &self.names);
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap_2()
                    .px_3()
                    .pt_3()
                    .pb_3()
                    .child(widgets::section_title(t, view.title.clone()))
                    .children(view.banner.clone().map(|banner| div().text_sm().text_color(rgb(t.text)).child(banner)))
                    .when(!view.facts.is_empty(), |body| {
                        body.child(div().flex().flex_col().gap_0p5().text_xs().children(view.facts.iter().map(|(label, value)| {
                            div().flex().gap_2().child(div().flex_none().w(px(104.)).text_color(rgb(t.muted)).child(*label)).child(div().text_color(rgb(t.text)).child(value.clone()))
                        })))
                    })
                    .children(view.sections.iter().map(|section| self.render_card_section(section)))
                    .children(view.boards.iter().map(|board| self.render_board_box(board)))
            }
        };
        island(t)
            .flex_none()
            .w(px(self.detail_width as f32))
            .child(div().id("detail-body").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.detail_scroll).children(snapshot).child(event))
            .child(widgets::scrollbar(t, "detail-scrollbar", &self.detail_scroll))
    }

    /// A combat board as the snapshot draws it, in a box like the Live section's opponent. The
    /// opponent's skills sit over their items and the player's under theirs, as the game shows them
    /// facing each other. The row scrolls sideways when the island is narrower than the ten slots.
    fn render_board_box(&self, board: &BoardBox) -> Div {
        let t = self.theme;
        let opponent = board.title == "OPPONENT";
        let prefix = if opponent { "detail-opponent" } else { "detail-player" };
        let skills = (!board.skills.is_empty()).then(|| self.render_skills(prefix, &board.skills));
        let (above, below) = if opponent { (skills, None) } else { (None, skills) };
        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(t.highlight_med))
            .child(div().text_xs().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child(board.title))
            .children(above)
            .child(self.render_grid(prefix.to_owned(), &board.board, if opponent { MarkSide::Above } else { MarkSide::Below }))
            .children(below)
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

    pub(in crate::ui) fn render_card(&self, card: &CardView) -> Div {
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
