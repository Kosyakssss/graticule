//! Charts: lines, bars and candles drawn on a canvas, with scales whose labels sit on whole cells.

use std::sync::Arc;

use crate::canvas::{Drawing, Weight};
use crate::ink::Ink;

/// One period of a price: where it opened, its highest and lowest, and where it closed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum Series {
    Line(Vec<(f64, f64)>, Ink),
    Bars(Vec<(f64, f64)>, Ink),
    Candles(Vec<Candle>),
}

/// What a chart shows and how many rows tall: its series, drawn as soon as each is added, so a plot made in a derived value or a job is drawn there, off the main thread. `ui.chart(t!("Price"), &self.plot)` shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plot {
    rows: i32,
    series: Vec<Series>,
    names: Vec<String>,
    drawing: Arc<Drawing>,
    x: Vec<(f32, String)>,
    y: Vec<(f32, String)>,
}

impl Plot {
    /// An empty plot `rows` tall. Its scale is chosen so each labelled value falls in the middle of a row.
    pub fn new(rows: i32) -> Plot {
        Plot { rows: rows.max(2), series: Vec::new(), names: Vec::new(), drawing: Arc::new(Drawing::new()), x: Vec::new(), y: Vec::new() }
    }

    pub fn rows(&self) -> i32 {
        self.rows
    }

    /// A line through `points`, each an x and a y, in `ink`.
    pub fn line(mut self, points: impl IntoIterator<Item = (f64, f64)>, ink: Ink) -> Plot {
        self.series.push(Series::Line(points.into_iter().collect(), ink));
        self.drawn()
    }

    /// A bar from zero to y at each x, in `ink`.
    pub fn bars(mut self, bars: impl IntoIterator<Item = (f64, f64)>, ink: Ink) -> Plot {
        self.series.push(Series::Bars(bars.into_iter().collect(), ink));
        self.drawn()
    }

    /// A candle at each place from 0, in the ok ink where the price closed higher than it opened and the danger ink where it closed lower.
    pub fn candles(mut self, candles: impl IntoIterator<Item = Candle>) -> Plot {
        self.series.push(Series::Candles(candles.into_iter().collect()));
        self.drawn()
    }

    /// Names for the places along the x axis, 0, 1, 2 and on, in place of the numbers: months, days, symbols.
    pub fn names(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Plot {
        self.names = names.into_iter().map(Into::into).collect();
        self.drawn()
    }

    pub(crate) fn drawing(&self) -> &Arc<Drawing> {
        &self.drawing
    }

    /// The labels along the bottom, each with where it falls from 0 at the left to 1 at the right.
    pub(crate) fn x_labels(&self) -> &[(f32, String)] {
        &self.x
    }

    /// The labels up the side, each with where it falls from 0 at the bottom to 1 at the top.
    pub(crate) fn y_labels(&self) -> &[(f32, String)] {
        &self.y
    }

    fn ranges(&self) -> Option<((f64, f64), (f64, f64))> {
        let (mut x, mut y) = ((f64::INFINITY, f64::NEG_INFINITY), (f64::INFINITY, f64::NEG_INFINITY));
        let mut take = |px: f64, py: f64| {
            if px.is_finite() && py.is_finite() {
                (x.0, x.1, y.0, y.1) = (x.0.min(px), x.1.max(px), y.0.min(py), y.1.max(py));
            }
        };
        for s in &self.series {
            match s {
                Series::Line(points, _) => points.iter().for_each(|&(a, b)| take(a, b)),
                Series::Bars(bars, _) => bars.iter().for_each(|&(a, b)| {
                    take(a - 0.5, b);
                    take(a + 0.5, 0.0);
                }),
                Series::Candles(candles) => candles.iter().enumerate().for_each(|(i, c)| {
                    take(i as f64 - 0.5, c.low);
                    take(i as f64 + 0.5, c.high);
                }),
            }
        }
        (x.0 <= x.1 && y.0 <= y.1).then_some((x, y))
    }

    fn drawn(mut self) -> Plot {
        let mut d = Drawing::new();
        let Some((x, y)) = self.ranges() else {
            (self.drawing, self.x, self.y) = (Arc::new(d), Vec::new(), Vec::new());
            return self;
        };
        let (ys, y_step, y0, y1) = rows_of(y.0, y.1, self.rows);
        let (x0, x1) = if x.1 > x.0 { x } else { (x.0 - 1.0, x.0 + 1.0) };
        d.space(x0 as f32..x1 as f32, y1 as f32..y0 as f32);
        for &t in ys.iter().filter(|t| **t > y0 && **t < y1) {
            d.line((x0 as f32, t as f32), (x1 as f32, t as f32)).ink(Ink::Rule).stroke(Weight::Hair);
        }
        for s in &self.series {
            match s {
                Series::Line(points, ink) => {
                    d.polyline(points.iter().map(|&(a, b)| (a as f32, b as f32))).ink(*ink).stroke(Weight::Bold);
                }
                Series::Bars(bars, ink) => {
                    for &(a, b) in bars {
                        d.rect(((a - 0.4) as f32, 0.0), ((a + 0.4) as f32, b as f32)).ink(*ink).fill();
                    }
                }
                Series::Candles(candles) => {
                    for (i, c) in candles.iter().enumerate() {
                        let (at, ink) = (i as f32, if c.close >= c.open { Ink::Ok } else { Ink::Danger });
                        d.line((at, c.low as f32), (at, c.high as f32)).ink(ink).stroke(Weight::Line);
                        d.rect((at - 0.3, c.open as f32), (at + 0.3, c.close as f32)).ink(ink).fill();
                    }
                }
            }
        }
        let place = |v: f64, lo: f64, hi: f64| ((v - lo) / (hi - lo)) as f32;
        self.y = ys.iter().map(|&t| (place(t, y0, y1), number(t, y_step))).collect();
        self.x = if self.names.is_empty() {
            let (xs, x_step) = ticks(x0, x1, 6);
            xs.into_iter().filter(|t| *t >= x0 && *t <= x1).map(|t| (place(t, x0, x1), number(t, x_step))).collect()
        } else {
            self.names.iter().enumerate().filter(|(i, _)| (*i as f64) >= x0 && (*i as f64) <= x1).map(|(i, n)| (place(i as f64, x0, x1), n.clone())).collect()
        };
        self.drawing = Arc::new(d);
        self
    }
}

/// Round numbers for a scale `rows` tall that holds `lo` to `hi`, each in the middle of a row, every other row when there are six or more; the step between them; and the values at the bottom and top edges.
fn rows_of(lo: f64, hi: f64, rows: i32) -> (Vec<f64>, f64, f64, f64) {
    let (rows, every) = (f64::from(rows.max(2)), if rows >= 6 { 2.0 } else { 1.0 });
    let span = (hi - lo).abs().max(f64::EPSILON);
    let mut power = 10f64.powf((span / rows).log10().floor() - 1.0);
    let step = loop {
        if let Some(s) = [1.0, 2.0, 5.0].into_iter().map(|m| m * power).find(|s| (lo / s).floor() * s + (rows - 1.0) * s / every >= hi) {
            break s;
        }
        power *= 10.0;
    };
    let (first, per_row) = ((lo / step).floor() * step, step / every);
    let (bottom, top) = (first - per_row / 2.0, first + (rows - 0.5) * per_row);
    let ticks = (0..).map(|k| first + f64::from(k) * step).take_while(|t| *t < top).collect();
    (ticks, step, bottom, top)
}

/// Round numbers from at most `lo` to at least `hi`, about `count` of them, and the step between them: 1, 2 or 5 times a power of ten.
fn ticks(lo: f64, hi: f64, count: usize) -> (Vec<f64>, f64) {
    let span = (hi - lo).abs().max(f64::EPSILON);
    let rough = span / count.max(1) as f64;
    let power = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * power).find(|s| *s >= rough).unwrap_or(10.0 * power);
    let (first, last) = ((lo / step).floor() as i64, (hi / step).ceil() as i64);
    ((first..=last).map(|k| k as f64 * step).collect(), step)
}

fn number(v: f64, step: f64) -> String {
    let places = if step >= 1.0 { 0 } else { (-step.log10().floor()) as usize };
    if places == 0 && v.abs() < 1e15 { crate::value::grouped(v.round() as i64) } else { format!("{v:.places$}") }
}

/// How a sparkline draws its values: as bars from the lowest, each filling its share of a cell's height.
pub(crate) fn spark_heights(values: &[f64]) -> Vec<f32> {
    let (lo, hi) = values.iter().filter(|v| v.is_finite()).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    if lo > hi {
        return Vec::new();
    }
    values.iter().map(|&v| if hi > lo { (0.15 + 0.85 * (v - lo) / (hi - lo)) as f32 } else { 0.5 }).collect()
}
