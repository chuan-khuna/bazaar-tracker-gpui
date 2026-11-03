//! The Run's economy under the board: a line of the gold the player had at the start of each hour,
//! split into days by alternating bands, with a tick under each hour and the days named along the
//! bottom. Hovering an hour shows its gold and income.

use gpui::{Context, Div, div, prelude::*, px, relative, rgb, rgba};
use gpui_component::chart::LineChart;

use crate::ui::TrackerApp;
use crate::ui::widgets;
use crate::tracking::run::economy::{self, HourGold};

/// How far a day's band reaches past its first and last hour, in hours: half the gap to the next.
const BAND_REACH: f32 = 0.5;

/// How tall the drawing is, without the day labels under it.
const CHART_HEIGHT: f32 = 106.;

/// The width of the gold labels on the left.
const AXIS_WIDTH: f32 = 32.;

/// The steps the gold scale can climb by, smallest first.
const GOLD_STEPS: [i32; 4] = [5, 10, 25, 50];

/// The gaps between the gold ticks: five ticks, from 0 to the top, unless the peak needs more.
const GOLD_GAPS: i32 = 4;

/// The gold ticks, from 0 up: the smallest step in [`GOLD_STEPS`] whose four gaps reach `peak`.
/// Above 200 the step stays at 50 and ticks are added until they reach it.
fn gold_ticks(peak: i32) -> Vec<i32> {
    let peak = peak.max(0);
    let step = GOLD_STEPS.into_iter().find(|step| step * GOLD_GAPS >= peak).unwrap_or(GOLD_STEPS[GOLD_STEPS.len() - 1]);
    let gaps = ((peak + step - 1) / step).max(GOLD_GAPS);
    (0..=gaps).map(|gap| gap * step).collect()
}

/// A tick label is this tall: it is drawn centred on its tick.
const TICK_LABEL_HEIGHT: f32 = 12.;

impl TrackerApp {
    /// The ECONOMY section and the SHOP ODDS column beside it, 70% and 30% of the panel's width.
    pub(in crate::ui) fn render_economy_row(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_none()
            .items_start()
            .w_full()
            .map(|row| match self.render_economy() {
                Some(economy) => row.child(economy),
                None => row.child(div().flex_1()),
            })
            .child(self.render_upcoming_odds(cx))
    }

    /// The ECONOMY section, or nothing while the Run has fewer than two hours with gold.
    pub(in crate::ui) fn render_economy(&self) -> Option<Div> {
        let t = self.theme;
        let points = economy::by_hour(self.session.events());
        if points.len() < 2 {
            return None;
        }
        let peak = points.iter().map(|point| point.gold).max().unwrap_or(0);
        let ticks = gold_ticks(peak);
        let top = ticks.last().copied().unwrap_or(1).max(1);
        // Each tick's height from the top of the chart, as a share of its height.
        let tick_at = |value: i32| 1. - value as f32 / top as f32;
        let span = (points.len() - 1) as f32;
        // Each point's place along the line, as a share of its width.
        let at = |index: usize| index as f32 / span;
        // Each day's band: from half an hour before its first hour to half an hour after its last.
        let bands: Vec<(i32, f32, f32)> = economy::days(&points)
            .into_iter()
            .map(|day| (day.day, ((day.first as f32 - BAND_REACH) / span).max(0.), ((day.last as f32 + BAND_REACH) / span).min(1.)))
            .collect();
        let hours: Vec<f32> = (0..points.len()).map(at).collect();
        let label = |text: String| div().text_size(px(10.)).text_color(rgb(t.muted)).child(text);
        let (gold, muted) = (t.gold, t.muted);
        // Every other day is shaded, starting with the second.
        let shade = (t.highlight_med << 8) | 0x60;
        let chart = LineChart::new(points)
            .x(|point: &HourGold| format!("D{} H{}", point.day, point.hour))
            .y(|point: &HourGold| f64::from(point.gold))
            .y_domain(0., f64::from(top))
            .y_padding(0., 0.)
            .grid(false)
            .x_axis(false)
            .linear()
            .stroke(rgb(t.gold))
            .appear(false)
            .tooltip_content(move |point, _, _| {
                let row = |name: &'static str, value: String| div().flex().justify_between().gap_3().child(div().text_color(rgb(muted)).child(name)).child(div().text_color(rgb(gold)).child(value));
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(format!("Day {} · Hour {}", point.day, point.hour)))
                    .child(row("Gold", point.gold.to_string()))
                    .children(point.income.map(|income| row("Income", format!("{income:+}"))))
            });
        Some(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .flex_1()
                .min_w_0()
                .px_3()
                .py_2()
                .child(widgets::section_header(
                    [widgets::section_title(t, "ECONOMY"), div().text_xs().text_color(rgb(t.subtle)).child("gold at the start of each hour")],
                    [],
                ))
                .child(
                    div()
                        .flex()
                        .w_full()
                        .child(div().relative().flex_none().w(px(AXIS_WIDTH)).h(px(CHART_HEIGHT)).children(ticks.iter().map(|value| {
                            div()
                                .absolute()
                                .left_0()
                                .right(px(6.))
                                .top(relative(tick_at(*value)))
                                .mt(px(-TICK_LABEL_HEIGHT / 2.))
                                .h(px(TICK_LABEL_HEIGHT))
                                .flex()
                                .items_center()
                                .justify_end()
                                .child(label(value.to_string()))
                        })))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    div()
                                        .relative()
                                        .w_full()
                                        .h(px(CHART_HEIGHT))
                                        .children(bands.iter().enumerate().filter(|(index, _)| index % 2 == 1).map(|(_, (_, from, to))| div().absolute().top_0().bottom_0().left(relative(*from)).w(relative(to - from)).bg(rgba(shade))))
                                        .children(ticks.iter().map(|value| div().absolute().left_0().right_0().top(relative(tick_at(*value))).h(px(1.)).bg(rgb(t.overlay))))
                                        .child(div().absolute().inset_0().child(chart)),
                                )
                                .child(div().relative().w_full().h(px(4.)).children(hours.into_iter().map(|x| div().absolute().top_0().left(relative(x)).w(px(1.)).h(px(4.)).bg(rgb(t.subtle)))))
                                .child(
                                    div()
                                        .relative()
                                        .w_full()
                                        .h(px(12.))
                                        .children(bands.into_iter().map(|(day, from, to)| div().absolute().top_0().left(relative(from)).w(relative(to - from)).flex().justify_center().child(label(format!("D{day}"))))),
                                ),
                        ),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::gold_ticks;

    #[test]
    fn five_ticks_climb_by_the_smallest_step_that_reaches_the_peak() {
        assert_eq!(gold_ticks(0), [0, 5, 10, 15, 20]);
        assert_eq!(gold_ticks(20), [0, 5, 10, 15, 20]);
        assert_eq!(gold_ticks(21), [0, 10, 20, 30, 40]);
        assert_eq!(gold_ticks(41), [0, 25, 50, 75, 100]);
        assert_eq!(gold_ticks(128), [0, 50, 100, 150, 200]);
        assert_eq!(gold_ticks(200), [0, 50, 100, 150, 200]);
    }

    #[test]
    fn above_200_the_step_stays_at_50_and_ticks_are_added() {
        assert_eq!(gold_ticks(201), [0, 50, 100, 150, 200, 250]);
        assert_eq!(gold_ticks(340), [0, 50, 100, 150, 200, 250, 300, 350]);
    }
}
