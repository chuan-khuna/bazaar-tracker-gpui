//! The Run's economy under the board: a line of the gold the player had at the start of each hour,
//! split into days by alternating bands, with a tick under each hour and the days named along the
//! bottom. Hovering an hour shows its gold and income.

use gpui::{Div, div, prelude::*, px, relative, rgb, rgba};
use gpui_component::chart::LineChart;

use crate::ui::TrackerApp;
use super::board::MAX_ROW_WIDTH;
use crate::tracking::run::economy::{self, HourGold};

/// How far a day's band reaches past its first and last hour, in hours: half the gap to the next.
const BAND_REACH: f32 = 0.5;

/// How tall the drawing is, without the day labels under it.
const CHART_HEIGHT: f32 = 96.;

/// The width of the gold labels on the left.
const AXIS_WIDTH: f32 = 32.;

/// The top of the scale: `max` rounded up to a multiple of ten, and never 0.
fn scale_top(max: i32) -> i32 {
    ((max.max(1) + 9) / 10) * 10
}

impl TrackerApp {
    /// The ECONOMY section, or nothing while the Run has fewer than two hours with gold.
    pub(in crate::ui) fn render_economy(&self) -> Option<Div> {
        let t = self.theme;
        let points = economy::by_hour(self.session.events());
        if points.len() < 2 {
            return None;
        }
        let peak = points.iter().map(|point| point.gold).max().unwrap_or(0);
        let top = scale_top(peak);
        let last = points.last().map_or(0, |point| point.gold);
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
            .y_tick_count(3)
            .grid_dashed(false)
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
                .flex_none()
                .gap_1p5()
                .px_3()
                .py_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .text_xs()
                        .child(div().font_weight(gpui::FontWeight::BOLD).text_color(rgb(t.muted)).child("ECONOMY"))
                        .child(div().text_color(rgb(t.subtle)).child("gold at the start of each hour"))
                        .child(div().ml_auto().text_color(rgb(t.gold)).child(format!("peak {peak} · now {last}"))),
                )
                .child(
                    div()
                        .flex()
                        .w_full()
                        .max_w(px(MAX_ROW_WIDTH))
                        .child(div().flex().flex_col().flex_none().justify_between().w(px(AXIS_WIDTH)).h(px(CHART_HEIGHT)).child(label(top.to_string())).child(label("0".to_owned())))
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
    use super::scale_top;

    #[test]
    fn the_scale_tops_out_at_the_next_ten() {
        assert_eq!([scale_top(0), scale_top(1), scale_top(10), scale_top(41)], [10, 10, 10, 50]);
    }
}
