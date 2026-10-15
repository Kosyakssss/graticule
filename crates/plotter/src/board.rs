use std::collections::HashSet;

use graticule_macros::Facts;
use ledger::{Key, List, Outcome};

use crate::geometry::{Bounds, Geometry, Point};
use crate::order;

/// A board: its shapes and the bindings between them. Every change goes through its methods, which keep bound arrows on what they point at within the same change, so one undo takes back both.
#[derive(Facts, Default)]
pub struct Board {
    pub shapes: List<Shape>,
    pub bindings: List<Binding>,
}

/// One shape on the board. Its box is `x`, `y`, `w` and `h` in page units; an arrow runs from (`x`, `y`) to (`x + w`, `y + h`), so its width and height may be negative.
#[derive(Facts, Debug, PartialEq)]
pub struct Shape {
    #[key]
    pub id: Key<Shape>,
    /// Where it sits in the stack: shapes are drawn in the order of these keys, from [`order::between`].
    pub index: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub kind: Kind,
    pub ink: Ink,
    pub locked: bool,
}

/// What a shape is.
#[derive(Facts, Debug, Default, PartialEq)]
pub enum Kind {
    #[default]
    Rect,
    Ellipse,
    Text(String),
    /// A freehand stroke: its points as x and y in turn, relative to the shape's corner.
    Draw(Vec<f64>),
    Arrow,
}

/// How tall a line of a text shape is, in page units.
pub const LINE: f64 = 16.0;

/// How wide `words` are in a text shape, in page units: monospace, so a letter is always as wide.
pub fn text_width(words: &str) -> f64 {
    words.chars().count() as f64 * 8.4
}

/// The semantic ink a shape is drawn in; the theme decides the color.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ink {
    #[default]
    Ink,
    Muted,
    Link,
    Ok,
    Warn,
    Danger,
}

/// An arrow's end held to another shape: where on that shape it points, as a fraction of the shape's box, so the end follows it through any move or resize.
#[derive(Facts, Debug)]
pub struct Binding {
    #[key]
    pub id: Key<Binding>,
    pub arrow: Key<Shape>,
    pub end: End,
    pub to: Key<Shape>,
    pub ax: f64,
    pub ay: f64,
}

/// Which end of an arrow.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum End {
    #[default]
    Start,
    End,
}

impl Shape {
    pub fn new(id: Key<Shape>, kind: Kind, bounds: Bounds) -> Self {
        Shape { id, index: String::new(), x: bounds.x, y: bounds.y, w: bounds.w, h: bounds.h, kind, ink: Ink::default(), locked: false }
    }

    /// An arrow from `from` to `to`.
    pub fn arrow(id: Key<Shape>, from: Point, to: Point) -> Self {
        Shape { id, index: String::new(), x: from.x, y: from.y, w: to.x - from.x, h: to.y - from.y, kind: Kind::Arrow, ink: Ink::default(), locked: false }
    }

    pub fn bounds(&self) -> Bounds {
        self.geometry().bounds()
    }

    /// Where an arrow's end is.
    pub fn end(&self, end: End) -> Point {
        match end {
            End::Start => Point::new(self.x, self.y),
            End::End => Point::new(self.x + self.w, self.y + self.h),
        }
    }

    pub fn geometry(&self) -> Geometry {
        let b = Bounds::new(self.x, self.y, self.w, self.h);
        match &self.kind {
            Kind::Rect | Kind::Text(_) => Geometry::Rect(b),
            Kind::Ellipse => Geometry::Ellipse(b),
            Kind::Arrow => Geometry::Line(vec![self.end(End::Start), self.end(End::End)]),
            Kind::Draw(points) => Geometry::Line(points.chunks_exact(2).map(|p| Point::new(self.x + p[0], self.y + p[1])).collect()),
        }
    }

    /// A freehand stroke's outline, to fill: see [`crate::freehand`]. Other shapes have none.
    pub fn stroke(&self) -> Option<Vec<Point>> {
        let Kind::Draw(points) = &self.kind else { return None };
        let points: Vec<Point> = points.chunks_exact(2).map(|p| Point::new(self.x + p[0], self.y + p[1])).collect();
        Some(crate::freehand::outline(&points, crate::freehand::SIZE))
    }

    fn place_end(&mut self, end: End, at: Point) {
        let (start, finish) = match end {
            End::Start => (at, self.end(End::End)),
            End::End => (self.end(End::Start), at),
        };
        (self.x, self.y, self.w, self.h) = (start.x, start.y, finish.x - start.x, finish.y - start.y);
    }
}

impl Binding {
    /// Where the bound end belongs on the shape it is held to.
    pub fn anchor(&self, to: &Shape) -> Point {
        let b = to.bounds();
        Point::new(b.x + self.ax * b.w, b.y + self.ay * b.h)
    }
}

impl Board {
    /// The shapes from the bottom of the stack to the top.
    pub fn stacked(&self) -> Vec<&Shape> {
        let mut shapes: Vec<&Shape> = self.shapes.iter().collect();
        shapes.sort_by(|a, b| a.index.cmp(&b.index));
        shapes
    }

    /// Puts a shape on top of the stack.
    pub fn add(&mut self, mut shape: Shape) -> Outcome {
        let top = self.shapes.iter().map(|s| s.index.as_str()).max().map(str::to_string);
        shape.index = order::between(top.as_deref(), None);
        self.shapes.push(shape)
    }

    /// Gives a text shape new words, as wide as they need. Nothing happens to a locked shape, a shape that holds no text, or words it already holds.
    pub fn write(&mut self, key: Key<Shape>, words: &str) -> Outcome {
        let writable = self.shapes.find(key).is_some_and(|s| !s.locked && matches!(&s.kind, Kind::Text(t) if t != words));
        if !writable {
            return Outcome::Nothing;
        }
        self.shapes.change(key, |s| {
            s.kind = Kind::Text(words.to_string());
            s.w = text_width(words);
        })
    }

    /// Moves shapes by `dx` and `dy`, leaving locked ones where they are.
    pub fn translate(&mut self, keys: &[Key<Shape>], dx: f64, dy: f64) -> Outcome {
        let mut outcome = Outcome::Nothing;
        for &key in keys {
            if self.shapes.find(key).is_some_and(|s| !s.locked) && (dx != 0.0 || dy != 0.0) {
                outcome = self.shapes.change(key, |s| (s.x, s.y) = (s.x + dx, s.y + dy));
            }
        }
        self.settle(keys);
        outcome
    }

    /// Gives a shape a new box. A locked shape keeps its own.
    pub fn resize(&mut self, key: Key<Shape>, to: Bounds) -> Outcome {
        if !self.shapes.find(key).is_some_and(|s| !s.locked) {
            return Outcome::Nothing;
        }
        let outcome = self.shapes.change(key, |s| {
            if let Kind::Draw(points) = &mut s.kind {
                let (sx, sy) = (if s.w == 0.0 { 1.0 } else { to.w / s.w }, if s.h == 0.0 { 1.0 } else { to.h / s.h });
                for p in points.chunks_exact_mut(2) {
                    (p[0], p[1]) = (p[0] * sx, p[1] * sy);
                }
            }
            (s.x, s.y, s.w, s.h) = (to.x, to.y, to.w, to.h);
        });
        self.settle(&[key]);
        outcome
    }

    /// Takes shapes off the board, with every binding to or from them. An arrow whose target goes keeps its end where it was.
    pub fn delete(&mut self, keys: &[Key<Shape>]) -> Outcome {
        for b in self.touching(keys) {
            let _ = self.bindings.remove(b);
        }
        let mut outcome = Outcome::Nothing;
        for &key in keys {
            if self.shapes.remove(key).changed() {
                outcome = Outcome::Changed;
            }
        }
        outcome
    }

    /// Holds an arrow's end to a shape at the point `at`, which the end moves to. An end already held is held anew. Arrows can't be held to, so moving one end never pulls on another arrow.
    pub fn bind(&mut self, id: Key<Binding>, arrow: Key<Shape>, end: End, to: Key<Shape>, at: Point) -> Outcome {
        let (Some(target), true) = (self.shapes.find(to).filter(|t| t.kind != Kind::Arrow), self.shapes.find(arrow).is_some_and(|a| a.kind == Kind::Arrow)) else { return Outcome::Nothing };
        let b = target.bounds();
        let fraction = |v: f64, start: f64, size: f64| if size == 0.0 { 0.5 } else { ((v - start) / size).clamp(0.0, 1.0) };
        let (ax, ay) = (fraction(at.x, b.x, b.w), fraction(at.y, b.y, b.h));
        let _ = self.unbind(arrow, end);
        let _ = self.bindings.push(Binding { id, arrow, end, to, ax, ay });
        self.settle(&[to]);
        Outcome::Changed
    }

    /// Lets go of an arrow's end, leaving it where it is.
    pub fn unbind(&mut self, arrow: Key<Shape>, end: End) -> Outcome {
        let held: Vec<_> = self.bindings.linked(Binding::arrow, arrow).filter(|b| b.end == end).map(|b| b.id).collect();
        let mut outcome = Outcome::Nothing;
        for b in held {
            outcome = self.bindings.remove(b);
        }
        outcome
    }

    /// Moves shapes to the top of the stack, keeping their order among themselves.
    pub fn to_front(&mut self, keys: &[Key<Shape>]) -> Outcome {
        self.restack(keys, true)
    }

    /// Moves shapes to the bottom of the stack, keeping their order among themselves.
    pub fn to_back(&mut self, keys: &[Key<Shape>]) -> Outcome {
        self.restack(keys, false)
    }

    fn restack(&mut self, keys: &[Key<Shape>], front: bool) -> Outcome {
        let moving: HashSet<Key<Shape>> = keys.iter().copied().filter(|k| self.shapes.has(*k)).collect();
        let rest = self.shapes.iter().filter(|s| !moving.contains(&s.id)).map(|s| s.index.as_str());
        let edge = if front { rest.max() } else { rest.min() }.map(str::to_string);
        let stacked: Vec<&Shape> = self.stacked().into_iter().filter(|s| moving.contains(&s.id)).collect();
        let there = match (&edge, front) {
            (None, _) => true,
            (Some(edge), true) => stacked.iter().all(|s| s.index > *edge),
            (Some(edge), false) => stacked.iter().all(|s| s.index < *edge),
        };
        if there {
            return Outcome::Nothing;
        }
        let stacked: Vec<Key<Shape>> = stacked.into_iter().map(|s| s.id).collect();
        let indexes = if front { order::run(edge.as_deref(), None, stacked.len()) } else { order::run(None, edge.as_deref(), stacked.len()) };
        let mut outcome = Outcome::Nothing;
        for (key, index) in stacked.into_iter().zip(indexes) {
            if self.shapes.change(key, |s| s.index = index).changed() {
                outcome = Outcome::Changed;
            }
        }
        outcome
    }

    /// The topmost shape at `p`, within `margin` of a line.
    pub fn hit(&self, p: Point, margin: f64) -> Option<Key<Shape>> {
        self.stacked().into_iter().rev().find(|s| s.geometry().hit(p, margin)).map(|s| s.id)
    }

    /// The shapes wholly inside a box, as a drag from corner to corner chooses them.
    pub fn inside(&self, area: Bounds) -> Vec<Key<Shape>> {
        self.shapes.iter().filter(|s| area.holds(&s.bounds())).map(|s| s.id).collect()
    }

    /// The bindings held to one of `keys` or belonging to one of them, found through the list's links, so it costs the bindings found.
    pub fn touching(&self, keys: &[Key<Shape>]) -> Vec<Key<Binding>> {
        let mut found: Vec<Key<Binding>> = keys.iter().flat_map(|&k| self.bindings.linked(Binding::to, k).chain(self.bindings.linked(Binding::arrow, k))).map(|b| b.id).collect();
        found.sort_unstable();
        found.dedup();
        found
    }

    /// Puts every arrow end held to one of `moved`, or belonging to one of them, where its binding says.
    fn settle(&mut self, moved: &[Key<Shape>]) {
        let due: Vec<(Key<Shape>, End, Point)> =
            self.touching(moved).into_iter().filter_map(|b| self.bindings.find(b)).filter_map(|b| self.shapes.find(b.to).map(|to| (b.arrow, b.end, b.anchor(to)))).collect();
        for (arrow, end, at) in due {
            if self.shapes.find(arrow).is_some_and(|a| a.end(end) != at) {
                let _ = self.shapes.change(arrow, |a| a.place_end(end, at));
            }
        }
    }
}
