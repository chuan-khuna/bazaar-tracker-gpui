//! The Run's economy under the board: a line of the gold the player had at the start of each hour,
//! with the days marked along the bottom.

use gpui::{Div, Hsla, canvas, div, prelude::*, px, quad, relative, rgb, size, transparent_black};
use gpui_base::plot::Curve;
use gpui_base::plot::shape::Line;

use crate::ui::TrackerApp;
use super::board::MAX_ROW_WIDTH;
use crate::tracking::run::economy::{self, HourGold};

/// How tall the drawing is, without the day labels under it.
const CHART_HEIGHT: f32 = 96.;

/// Room kept around the line inside the drawing, so the dots at its edges are not cut.
const INSET: f32 = 4.;

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
        // Where each day begins along the line, as a share of its width.
        let days: Vec<(i32, f32)> = points.iter().enumerate().filter(|(index, point)| *index == 0 || points[index - 1].day != point.day).map(|(index, point)| (point.day, index as f32 / span)).collect();
        let (line_colour, rule_colour) = (Hsla::from(rgb(t.gold)), Hsla::from(rgb(t.overlay)));
        let label = |text: String| div().text_size(px(10.)).text_color(rgb(t.muted)).child(text);
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
                                .child(div().w_full().h(px(CHART_HEIGHT)).child(canvas(|_, _, _| (), move |bounds, (), window, _| paint_line(bounds, &points, top, line_colour, rule_colour, window)).size_full()))
                                .child(div().relative().w_full().h(px(12.)).children(days.into_iter().map(|(day, at)| div().absolute().left(relative(at)).child(label(format!("D{day}")))))),
                        ),
                ),
        )
    }
}

/// The rules at the top, middle and bottom of the scale, then the line through the hours with a
/// dot on each.
fn paint_line(bounds: gpui::Bounds<gpui::Pixels>, points: &[HourGold], top: i32, line: Hsla, rule: Hsla, window: &mut gpui::Window) {
    let (width, height) = (f32::from(bounds.size.width) - 2. * INSET, f32::from(bounds.size.height) - 2. * INSET);
    for share in [0., 0.5, 1.] {
        let y = bounds.origin.y + px(INSET + share * height);
        window.paint_quad(quad(gpui::Bounds::new(gpui::point(bounds.origin.x, y), size(bounds.size.width, px(1.))), px(0.), rule, px(0.), transparent_black(), gpui::BorderStyle::default()));
    }
    let span = (points.len().max(2) - 1) as f32;
    let indexed: Vec<(usize, i32)> = points.iter().enumerate().map(|(index, point)| (index, point.gold)).collect();
    Line::new()
        .data(indexed)
        .x(move |(index, _)| Some(INSET + *index as f32 / span * width))
        .y(move |(_, gold)| Some(INSET + (1. - *gold as f32 / top as f32) * height))
        .stroke(line)
        .stroke_width(px(1.5))
        .curve(Curve::Linear)
        .dot()
        .dot_size(px(4.))
        .dot_fill(line)
        .paint(&bounds, window);
}

#[cfg(test)]
mod tests {
    use super::scale_top;

    #[test]
    fn the_scale_tops_out_at_the_next_ten() {
        assert_eq!([scale_top(0), scale_top(1), scale_top(10), scale_top(41)], [10, 10, 10, 50]);
    }
}
