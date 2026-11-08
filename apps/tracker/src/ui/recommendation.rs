//! The recommendation card under an Advisor reply, drawn from `tracking::advisor::recommendation`:
//! a row per option, best first (rank, art, name, size, tier, ✓ fits or ✕ doesn't fit, the reason,
//! and bars with their `n`), a popup of each count's source runs with mini Boards while the pointer
//! is on the count, the runs clicked there expanded under their row (full Board, skills, result and
//! a Bazaar DB link), and the source line with ⚠ when the data is thin.

use gpui::{Anchor, Context, Div, FontWeight, ObjectFit, SharedString, Stateful, anchored, deferred, div, img, prelude::*, px, relative, rgb};

use crate::tracking::advisor::recommendation::{self, Bar, Fit, Recommendation, Row, RowData, SourceRun};
use crate::tracking::run::board::Tier;
use crate::ui::TrackerApp;
use crate::ui::run::board::{MarkSide, tier_colour};
use crate::ui::widgets::{self, IconName, Tone};

/// The art's height; its width follows the card's size, as on the board (a slot is half as wide).
const ART_HEIGHT: f32 = 36.;

/// The width of a bar's label, so the bars line up.
const BAR_LABEL: f32 = 118.;

/// The most width a bar's track takes.
const BAR_TRACK: f32 = 120.;

/// The popup's width, and a mini Board's in it.
const POPUP_WIDTH: f32 = 272.;
const MINI_BOARD: f32 = 240.;

/// The tallest the popup grows before it scrolls.
const POPUP_MAX_HEIGHT: f32 = 340.;

impl TrackerApp {
    /// The card: its title, its rows best first, and the source line. `key` is unique in the tab.
    pub(in crate::ui) fn render_recommendation(&self, key: String, card: &Recommendation, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let title = if card.title.is_empty() { "SUGGEST".to_owned() } else { format!("SUGGEST · {}", card.title) };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(t.accent))
            .bg(rgb(t.background))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(t.accent))
                    .child(widgets::icon(IconName::Star, t.accent))
                    .child(title)
                    .when(card.pending, |header| header.child(div().font_weight(FontWeight::NORMAL).text_color(rgb(t.muted)).child("ranking…"))),
            )
            .children(card.rows.iter().map(|row| self.render_recommendation_row(&key, card, row, cx)))
            .children(card.source.as_ref().map(|source| {
                let colour = if source.thin { t.gold } else { t.muted };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .pt_1()
                    .border_t_1()
                    .border_color(rgb(t.highlight_low))
                    .text_xs()
                    .text_color(rgb(colour))
                    .when(source.thin, |line| line.child(widgets::icon(IconName::TriangleAlert, colour)))
                    .child(format!("source: {}", source.text))
            }))
    }

    /// One option: rank, art, name, size and tier, fit; the reason; its bars or "no data"; and the
    /// source runs expanded under it.
    fn render_recommendation_row(&self, card_key: &str, card: &Recommendation, row: &Row, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let row_key = format!("{card_key}.r{}", row.rank);
        let tier = row.tier.as_deref().and_then(Tier::parse);
        let border = tier_colour(t, tier);
        let slots = match row.size.as_deref() {
            Some("Medium") => 2.,
            Some("Large") => 3.,
            _ => 1.,
        };
        let art = div()
            .flex_none()
            .w(px(ART_HEIGHT * 0.5 * slots))
            .h(px(ART_HEIGHT))
            .rounded_sm()
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(t.highlight_low))
            .overflow_hidden()
            .children(self.names.art(row.id.as_deref()).map(|art| img(art).size_full().object_fit(ObjectFit::Fill)));
        let what = [row.size.clone(), row.tier.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let fit = match row.fit {
            Fit::Yes => Some(div().flex().items_center().gap_0p5().text_color(rgb(t.success)).child(widgets::icon(IconName::Check, t.success)).child("fits")),
            Fit::No => Some(div().flex().items_center().gap_0p5().text_color(rgb(t.error)).child(widgets::icon(IconName::X, t.error)).child("doesn't fit")),
            Fit::Unknown => None,
        };
        let numbers = match &row.data {
            RowData::Waiting => None,
            RowData::NoData => Some(div().text_xs().text_color(rgb(t.muted)).child(recommendation::NO_DATA).into_any_element()),
            RowData::Bars(bars) => Some(div().flex().flex_col().gap_0p5().children(bars.iter().enumerate().map(|(index, bar)| self.render_bar(format!("{row_key}.b{index}"), &row_key, card, bar, cx))).into_any_element()),
        };
        let expanded: Vec<&SourceRun> = row.run_ids().into_iter().filter(|id| self.advisor.is_open(&run_key(&row_key, id))).filter_map(|id| card.runs.get(id)).collect();
        div()
            .flex()
            .gap_2()
            .child(
                div()
                    .flex_none()
                    .w(px(18.))
                    .h(px(18.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(if row.rank == 1 { t.accent } else { t.overlay }))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(if row.rank == 1 { t.background } else { t.subtle }))
                    .child(row.rank.to_string()),
            )
            .child(art)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_x_2()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(rgb(t.text)).child(row.card.clone()))
                            .when(!what.is_empty(), |line| line.child(div().text_xs().text_color(rgb(border)).child(what)))
                            .children(fit.map(|fit| fit.text_xs())),
                    )
                    .when(!row.reason.is_empty(), |cell| cell.child(div().text_xs().italic().text_color(rgb(t.subtle)).child(format!("“{}”", row.reason))))
                    .children(numbers)
                    .children(expanded.into_iter().map(|run| self.render_expanded_run(&row_key, run, cx))),
            )
    }

    /// A bar: its label, the track, the count (hover for its source runs) and its note.
    fn render_bar(&self, bar_key: String, row_key: &str, card: &Recommendation, bar: &Bar, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let shown = self.advisor.hover.shown() == Some(bar_key.as_str());
        let hovered = bar_key.clone();
        let count = div()
            .id(SharedString::from(format!("count-{bar_key}")))
            .relative()
            .flex_none()
            .px_1()
            .rounded_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(t.accent))
            .when(shown, |count| count.bg(rgb(t.overlay)))
            .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                this.advisor.hover.count(&hovered, *on);
                cx.notify();
            }))
            .child(bar.count())
            .when(shown, |count| count.child(div().absolute().top_full().left_0().child(deferred(anchored().anchor(Anchor::TopLeft).snap_to_window().child(self.render_runs_popup(&bar_key, row_key, card, bar, cx))))));
        div()
            .flex()
            .items_center()
            .gap_1p5()
            .text_xs()
            .child(div().flex_none().w(px(BAR_LABEL)).text_color(rgb(t.subtle)).child(bar.label.clone()))
            .child(
                div()
                    .flex_1()
                    .max_w(px(BAR_TRACK))
                    .min_w(px(40.))
                    .h(px(6.))
                    .rounded_sm()
                    .bg(rgb(t.highlight_low))
                    .overflow_hidden()
                    .child(div().h_full().w(relative(bar.fraction())).rounded_sm().bg(rgb(t.accent))),
            )
            .child(count)
            .children(bar.note.clone().map(|note| div().min_w_0().truncate().text_color(rgb(t.muted)).child(format!("({note})"))))
    }

    /// The runs behind a count, each with its result and a mini Board; a click expands one in the
    /// card. It stays open while the pointer is on it.
    fn render_runs_popup(&self, bar_key: &str, row_key: &str, card: &Recommendation, bar: &Bar, cx: &mut Context<Self>) -> Stateful<Div> {
        let t = self.theme;
        let runs: Vec<(&SourceRun, bool)> = card.runs_of(bar).collect();
        let hovered = bar_key.to_owned();
        let header = format!("{} · {} · {} source run{}", bar.label, bar.count(), runs.len(), if runs.len() == 1 { "" } else { "s" });
        div()
            .id(SharedString::from(format!("popup-{bar_key}")))
            .occlude()
            .mt(px(-2.))
            .w(px(POPUP_WIDTH))
            .max_h(px(POPUP_MAX_HEIGHT))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(t.highlight_med))
            .bg(rgb(t.surface))
            .shadow_md()
            .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                this.advisor.hover.popup(&hovered, *on);
                cx.notify();
            }))
            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(rgb(t.muted)).child(header))
            .when(runs.is_empty(), |popup| popup.child(div().text_xs().text_color(rgb(t.muted)).child("No source runs to show.")))
            .children(runs.into_iter().map(|(run, taken)| {
                let key = run_key(row_key, &run.id);
                let open = self.advisor.is_open(&key);
                let overlay = t.overlay;
                div()
                    .id(SharedString::from(format!("popup-{bar_key}-{}", run.id)))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(open, |line| line.bg(rgb(overlay)))
                    .hover(move |style| style.bg(rgb(overlay)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.advisor.toggle(&key);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(rgb(t.subtle))
                            .child(widgets::icon(if open { IconName::ChevronDown } else { IconName::ChevronRight }, t.muted))
                            .child(div().flex_1().min_w_0().truncate().child(run.result()))
                            .when(taken, |line| line.child(widgets::badge("taken", t.success, Tone::Soft, t))),
                    )
                    .child(self.render_mini_grid(format!("mini-{bar_key}-{}", run.id), &run.board_grid(&self.names), MINI_BOARD))
            }))
    }

    /// A source run opened in the card: its result, a link to its Bazaar DB page, ✕ to close, then
    /// its full Board and skills.
    fn render_expanded_run(&self, row_key: &str, run: &SourceRun, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let key = run_key(row_key, &run.id);
        let id = format!("run-{row_key}-{}", run.id);
        let link = run.link().map(|url| {
            let url = url.to_owned();
            div()
                .id(SharedString::from(format!("{id}-link")))
                .flex()
                .items_center()
                .gap_0p5()
                .text_color(rgb(t.accent))
                .cursor_pointer()
                .tooltip(widgets::hint(t, vec![url.clone()]))
                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)))
                .child("Bazaar DB")
                .child(widgets::icon(IconName::ExternalLink, t.accent))
        });
        let close = widgets::icon_button(t, SharedString::from(format!("{id}-close")), widgets::icon(IconName::X, t.subtle)).on_click(cx.listener(move |this, _, _, cx| {
            this.advisor.toggle(&key);
            cx.notify();
        }));
        div()
            .flex()
            .flex_col()
            .gap_1()
            .mt_1()
            .p_1p5()
            .rounded_md()
            .border_1()
            .border_color(rgb(t.highlight_med))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .children(run.hero.clone().map(|hero| div().font_weight(FontWeight::SEMIBOLD).text_color(rgb(t.text)).child(hero)))
                    .child(div().flex_1().min_w_0().truncate().text_color(rgb(t.subtle)).child(run.result()))
                    .children(link)
                    .child(close),
            )
            .child(self.render_grid(format!("{id}-board"), &run.board_grid(&self.names), MarkSide::Below, None, cx))
            .child(self.render_skills(&id, &run.skill_chips(&self.names)))
    }
}

/// The key a source run is opened under, per row.
fn run_key(row_key: &str, run_id: &str) -> String {
    format!("{row_key}.run.{run_id}")
}
