//! The time axis: which stretch of time shows, how it is counted and marked, and where the playhead is.

use ledger::{Field, Value};

use crate::value::grouped;

/// The stretch of time a time axis shows: the time at its left edge and how much time each column holds, in the app's own unit (samples, milliseconds, ticks). Keep it in the facts, as `#[session] view: TimeView`; scrolling and zooming change it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimeView {
    pub start: i64,
    pub per_col: i64,
}

impl Default for TimeView {
    fn default() -> Self {
        TimeView { start: 0, per_col: 1 }
    }
}

impl TimeView {
    pub fn new(start: i64, per_col: i64) -> TimeView {
        TimeView { start, per_col: per_col.max(1) }
    }

    /// The time just past the right edge of `cols` columns.
    pub fn end(&self, cols: i32) -> i64 {
        self.start.saturating_add(self.per_col.saturating_mul(i64::from(cols)))
    }

    /// Twice as close, or twice as far when `closer` is false, keeping the time at `col` where it is.
    pub fn zoomed(&self, closer: bool, col: f64) -> TimeView {
        let per_col = if closer { (self.per_col / 2).max(1) } else { self.per_col.saturating_mul(2).min(i64::MAX / 1_000_000) };
        let at = self.start as f64 + col * self.per_col as f64;
        TimeView { start: (at - col * per_col as f64).round() as i64, per_col }
    }

    /// Moved by `cols` columns, later when positive.
    pub fn moved(&self, cols: i64) -> TimeView {
        TimeView { start: self.start.saturating_add(cols.saturating_mul(self.per_col)), ..*self }
    }

    /// Moved so `t` shows among `cols` columns, if it doesn't already.
    pub fn showing(&self, t: i64, cols: i32) -> TimeView {
        let width = self.per_col.saturating_mul(i64::from(cols.max(1)));
        if t < self.start {
            TimeView { start: t, ..*self }
        } else if t >= self.start.saturating_add(width) {
            TimeView { start: t.saturating_sub(width - self.per_col), ..*self }
        } else {
            *self
        }
    }
}

impl Field for TimeView {
    ledger::register!(TimeView);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::List(vec![Value::Int(self.start), Value::Int(self.per_col)])
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::List(parts) => match parts.as_slice() {
                [Value::Int(start), Value::Int(per_col)] => Some(TimeView::new(*start, *per_col)),
                _ => None,
            },
            _ => None,
        }
    }
}
impl ledger::Document for TimeView {}

/// How a time axis counts its unit, which decides where its marks fall and what they say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clock {
    /// Plain numbers, marked at 1, 2 and 5 times a power of ten.
    Plain,
    /// Hours, minutes and seconds, `per_second` units to a second.
    Seconds { per_second: i64 },
    /// Bars and beats, `per_beat` units to a beat and `beats_per_bar` beats to a bar.
    Beats { per_beat: i64, beats_per_bar: i64 },
}

/// How many columns apart the labelled marks are, at the least.
const LABELLED: i64 = 10;

impl Clock {
    fn steps(&self) -> Vec<i64> {
        let mut steps: Vec<i64> = match *self {
            Clock::Plain => (0..54).map(|k| [1, 2, 5][k % 3] * 10i64.pow(k as u32 / 3)).collect(),
            Clock::Seconds { per_second: s } => {
                let parts = [1, 2, 5, 10, 20, 50, 100, 200, 500].map(|ms| s * ms / 1000);
                let whole = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21_600, 43_200, 86_400].map(|k| s * k);
                parts.into_iter().chain(whole).collect()
            }
            Clock::Beats { per_beat: b, beats_per_bar: n } => {
                let bar = b * n.max(1);
                [b / 16, b / 8, b / 4, b / 2, b].into_iter().chain((0..12).map(|k| bar << k)).collect()
            }
        };
        steps.retain(|s| *s > 0);
        steps.dedup();
        steps
    }

    /// The time between labelled marks when each column holds `per_col`.
    pub fn step(&self, per_col: i64) -> i64 {
        let steps = self.steps();
        steps.iter().copied().find(|s| *s >= per_col.saturating_mul(LABELLED)).or(steps.last().copied()).unwrap_or(1)
    }

    /// The time between the small marks: the finest step at least two columns apart.
    pub fn minor(&self, per_col: i64) -> i64 {
        let major = self.step(per_col);
        self.steps().into_iter().filter(|s| *s < major && major % *s == 0).find(|s| *s >= per_col.saturating_mul(2)).unwrap_or(major)
    }

    /// `t` moved to the nearest small mark, as the playhead and dragged things snap.
    pub fn snap(&self, view: TimeView, t: i64) -> i64 {
        let m = self.minor(view.per_col);
        (t as f64 / m as f64).round() as i64 * m
    }

    /// What a mark at `t` says, as finely as marks `step` apart need.
    pub fn label(&self, t: i64, step: i64) -> String {
        match *self {
            Clock::Plain => grouped(t),
            Clock::Seconds { per_second: s } => {
                let sign = if t < 0 { "-" } else { "" };
                let t = t.unsigned_abs() as i64;
                let (secs, part) = (t / s, t % s);
                let (h, m, sec) = (secs / 3600, secs / 60 % 60, secs % 60);
                let clock = if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") };
                if step < s { format!("{sign}{clock}.{:03}", part * 1000 / s) } else { format!("{sign}{clock}") }
            }
            Clock::Beats { per_beat: b, beats_per_bar: n } => {
                let bar_len = b * n.max(1);
                let (bar, within) = (t.div_euclid(bar_len) + 1, t.rem_euclid(bar_len));
                let (beat, part) = (within / b + 1, within % b);
                if step >= bar_len && within == 0 {
                    bar.to_string()
                } else if step >= b && part == 0 {
                    format!("{bar}.{beat}")
                } else {
                    format!("{bar}.{beat}.{}", part * 4 / b + 1)
                }
            }
        }
    }
}
