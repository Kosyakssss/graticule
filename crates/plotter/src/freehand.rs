//! The outline of a freehand stroke, as perfect-freehand draws it: the points smoothed, and the line thinner where the hand moved fast, as a pen's ink thins.
//!
//! Ported from perfect-freehand by Steve Ruiz, under its license:
//!
//! MIT License
//!
//! Copyright (c) 2021 Stephen Ruiz Ltd
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use crate::geometry::Point;

/// How wide a freehand stroke is at its widest, in page units.
pub const SIZE: f64 = 6.0;

const THINNING: f64 = 0.5;
const SMOOTHING: f64 = 0.5;
const STREAMLINE: f64 = 0.5;
const PRESSURE_RATE: f64 = 0.275;
const HALF_TURN: f64 = std::f64::consts::PI + 0.0001;

struct Sample {
    point: Point,
    pressure: f64,
    vector: Point,
    distance: f64,
    running: f64,
}

/// The outline of a finished stroke through `points`, `size` page units wide at its widest: a closed polygon to fill.
pub fn outline(points: &[Point], size: f64) -> Vec<Point> {
    let samples = samples(points, size);
    if samples.is_empty() || size <= 0.0 {
        return Vec::new();
    }
    let total = samples[samples.len() - 1].running;
    let least = (size * SMOOTHING).powi(2);
    let simulated = |distance: f64, before: f64| {
        let speed = (distance / size).min(1.0);
        let rest = (1.0 - speed).min(1.0);
        (before + (rest - before) * (speed * PRESSURE_RATE)).min(1.0)
    };
    let mut before = samples.iter().take(10).fold(samples[0].pressure, |acc, s| (acc + simulated(s.distance, acc)) / 2.0);
    let mut radius = radius_at(size, samples[samples.len() - 1].pressure);
    let mut first_radius = None;
    let mut previous_vector = samples[0].vector;
    let (mut pl, mut pr) = (samples[0].point, samples[0].point);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut previous_sharp = false;
    for (i, s) in samples.iter().enumerate() {
        let last = i == samples.len() - 1;
        if !last && total - s.running < 3.0 {
            continue;
        }
        let pressure = simulated(s.distance, before);
        radius = radius_at(size, pressure).max(0.01);
        first_radius.get_or_insert(radius);
        let next_vector = if last { s.vector } else { samples[i + 1].vector };
        let next_dot = if last { 1.0 } else { dot(s.vector, next_vector) };
        let previous_dot = dot(s.vector, previous_vector);
        let sharp = previous_dot < 0.0 && !previous_sharp;
        let next_sharp = next_dot < 0.0;
        if sharp || next_sharp {
            let offset = perpendicular(previous_vector) * radius;
            for step in 0..=13 {
                let t = f64::from(step) / 13.0;
                pl = rotate(s.point - offset, s.point, HALF_TURN * t);
                left.push(pl);
                pr = rotate(s.point + offset, s.point, -HALF_TURN * t);
                right.push(pr);
            }
            if next_sharp {
                previous_sharp = true;
            }
            continue;
        }
        previous_sharp = false;
        if last {
            let offset = perpendicular(s.vector) * radius;
            left.push(s.point - offset);
            right.push(s.point + offset);
            continue;
        }
        let offset = perpendicular(lerp(next_vector, s.vector, next_dot)) * radius;
        let tl = s.point - offset;
        if i <= 1 || distance_squared(pl, tl) > least {
            left.push(tl);
            pl = tl;
        }
        let tr = s.point + offset;
        if i <= 1 || distance_squared(pr, tr) > least {
            right.push(tr);
            pr = tr;
        }
        before = pressure;
        previous_vector = s.vector;
    }
    let first = samples[0].point;
    let finish = if samples.len() > 1 { samples[samples.len() - 1].point } else { first + Point::new(1.0, 1.0) };
    if samples.len() == 1 {
        let start = first + unit(perpendicular(first - finish)) * -first_radius.unwrap_or(radius);
        return (1..=13).map(|step| rotate(start, first, HALF_TURN * 2.0 * f64::from(step) / 13.0)).collect();
    }
    let start_cap: Vec<Point> = match right.first() {
        Some(&r) => (0..=13).map(|step| rotate(r, first, HALF_TURN * f64::from(step) / 13.0)).collect(),
        None => Vec::new(),
    };
    let direction = perpendicular(samples[samples.len() - 1].vector * -1.0);
    let end_start = finish + direction * radius;
    let end_cap = (1..29).map(|step| rotate(end_start, finish, HALF_TURN * 3.0 * f64::from(step) / 29.0));
    left.into_iter().chain(end_cap).chain(right.into_iter().rev()).chain(start_cap).collect()
}

fn samples(points: &[Point], size: f64) -> Vec<Sample> {
    let mut points = points.to_vec();
    match points.len() {
        0 => return Vec::new(),
        1 => points.push(points[0] + Point::new(1.0, 1.0)),
        2 => {
            let (a, b) = (points[0], points[1]);
            points = (0..=4).map(|k| lerp(a, b, f64::from(k) / 4.0)).collect();
        }
        _ => {}
    }
    let t = 0.15 + (1.0 - STREAMLINE) * 0.85;
    let mut out = vec![Sample { point: points[0], pressure: 0.5, vector: Point::new(1.0, 1.0), distance: 0.0, running: 0.0 }];
    let (mut running, mut reached) = (0.0, false);
    let most = points.len() - 1;
    for (i, &p) in points.iter().enumerate().skip(1) {
        let before = out[out.len() - 1].point;
        let point = if i == most { p } else { lerp(before, p, t) };
        if point == before {
            continue;
        }
        let distance = point.distance(before);
        running += distance;
        if i < most && !reached {
            if running < size {
                continue;
            }
            reached = true;
        }
        out.push(Sample { point, pressure: 0.5, vector: unit(before - point), distance, running });
    }
    out[0].vector = out.get(1).map_or(Point::default(), |s| s.vector);
    out
}

fn radius_at(size: f64, pressure: f64) -> f64 {
    size * (0.5 - THINNING * (0.5 - pressure))
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn perpendicular(a: Point) -> Point {
    Point::new(a.y, -a.x)
}

fn lerp(a: Point, b: Point, t: f64) -> Point {
    a + (b - a) * t
}

fn unit(a: Point) -> Point {
    let length = a.length();
    if length == 0.0 { a } else { a * (1.0 / length) }
}

fn distance_squared(a: Point, b: Point) -> f64 {
    let d = a - b;
    dot(d, d)
}

fn rotate(a: Point, around: Point, angle: f64) -> Point {
    let (s, c) = angle.sin_cos();
    let p = a - around;
    Point::new(p.x * c - p.y * s + around.x, p.x * s + p.y * c + around.y)
}
