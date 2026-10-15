use std::collections::HashSet;

use graticule_macros::Facts;
use ledger::{Key, List, Outcome};

use crate::geometry::{Bounds, Geometry, Point};
use crate::order;

/// A board: its shapes, the bindings between them and the groups that hold them together. Every change goes through its methods, which keep bound arrows on what they point at and groups whole within the same change, so one undo takes back all of it.
#[derive(Facts, Default)]
pub struct Board {
    pub shapes: List<Shape>,
    pub bindings: List<Binding>,
    pub groups: List<Group>,
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
    /// The group it is in, if any.
    pub group: Option<Key<Group>>,
}

/// Shapes held together, so choosing, moving, copying or deleting one does it to them all. A group may sit in another, and always holds at least two things: shapes, groups or both.
#[derive(Facts, Debug)]
pub struct Group {
    #[key]
    pub id: Key<Group>,
    /// The group it is in, if any.
    pub group: Option<Key<Group>>,
}

/// One thing directly in a group: a shape or another group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Top {
    Shape(Key<Shape>),
    Group(Key<Group>),
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
        Shape { id, index: String::new(), x: bounds.x, y: bounds.y, w: bounds.w, h: bounds.h, kind, ink: Ink::default(), locked: false, group: None }
    }

    /// An arrow from `from` to `to`.
    pub fn arrow(id: Key<Shape>, from: Point, to: Point) -> Self {
        Shape { id, index: String::new(), x: from.x, y: from.y, w: to.x - from.x, h: to.y - from.y, kind: Kind::Arrow, ink: Ink::default(), locked: false, group: None }
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

    /// A board of only `keys`, the bindings between them and the groups they are in: what copying them takes. Making it records nothing, even inside a command, since it is a copy and not a change.
    pub fn excerpt(&self, keys: &[Key<Shape>]) -> Board {
        let inside: HashSet<Key<Shape>> = keys.iter().copied().filter(|k| self.shapes.has(*k)).collect();
        let apart = ledger::Transaction::begin();
        let mut part = Board::default();
        let mut groups = HashSet::new();
        for shape in self.stacked().into_iter().filter(|s| inside.contains(&s.id)) {
            groups.extend(self.above(shape.group));
            let _ = part.shapes.push(shape.clone());
        }
        for group in groups.into_iter().filter_map(|g| self.groups.find(g)) {
            let _ = part.groups.push(group.clone());
        }
        for b in self.touching(keys).into_iter().filter_map(|b| self.bindings.find(b)).filter(|b| inside.contains(&b.arrow) && inside.contains(&b.to)) {
            let _ = part.bindings.push(b.clone());
        }
        let _ = apart.end();
        part
    }

    /// Puts every shape of `other` on top of the stack, in its own order, moved by `dx` and `dy`, with new keys from `key`, its bindings between them and its groups, and says what the new shapes' keys are.
    pub fn paste(&mut self, other: &Board, dx: f64, dy: f64, mut key: impl FnMut() -> u128) -> Vec<Key<Shape>> {
        let regrouped: Vec<(Key<Group>, Key<Group>)> = other.groups.iter().map(|g| (g.id, Key::from_raw(key()))).collect();
        let group = |old: Option<Key<Group>>| old.and_then(|o| regrouped.iter().find(|(g, _)| *g == o)).map(|(_, n)| *n);
        for g in other.groups.iter() {
            let _ = self.groups.push(Group { id: group(Some(g.id)).unwrap_or(g.id), group: group(g.group) });
        }
        let mut made = Vec::new();
        for shape in other.stacked() {
            let id = Key::from_raw(key());
            let _ = self.add(Shape { id, x: shape.x + dx, y: shape.y + dy, group: group(shape.group), ..shape.clone() });
            made.push((shape.id, id));
        }
        let new = |old: Key<Shape>| made.iter().find(|(o, _)| *o == old).map(|(_, n)| *n);
        for b in other.bindings.iter() {
            if let (Some(arrow), Some(to)) = (new(b.arrow), new(b.to)) {
                let _ = self.bindings.push(Binding { id: Key::from_raw(key()), arrow, to, ..b.clone() });
            }
        }
        made.into_iter().map(|(_, n)| n).collect()
    }

    /// Holds shapes together in a new group `id`, with the groups they are in as wholes, and says whether it did: it takes at least two things to group.
    pub fn group(&mut self, keys: &[Key<Shape>], id: Key<Group>) -> Outcome {
        let mut tops: Vec<Top> = keys.iter().filter_map(|&k| self.shapes.find(k)).map(|s| self.outermost(s.group).map_or(Top::Shape(s.id), Top::Group)).collect();
        tops.sort_unstable();
        tops.dedup();
        if tops.len() < 2 {
            return Outcome::Nothing;
        }
        let _ = self.groups.push(Group { id, group: None });
        for top in tops {
            self.lift(top, Some(id));
        }
        Outcome::Changed
    }

    /// Takes apart the outermost groups the shapes are in, leaving what they held, groups inside them included, as it was.
    pub fn ungroup(&mut self, keys: &[Key<Shape>]) -> Outcome {
        let mut tops: Vec<Key<Group>> = keys.iter().filter_map(|&k| self.shapes.find(k)).filter_map(|s| self.outermost(s.group)).collect();
        tops.sort_unstable();
        tops.dedup();
        let mut outcome = Outcome::Nothing;
        for g in tops {
            for top in self.held(g) {
                self.lift(top, None);
            }
            outcome = self.groups.remove(g);
        }
        outcome
    }

    /// Every shape that goes with `keys`: each one, or if it is in a group, every shape in its outermost group. Choosing, moving and copying take this.
    pub fn whole(&self, keys: &[Key<Shape>]) -> Vec<Key<Shape>> {
        let mut seen = HashSet::new();
        let mut whole = Vec::new();
        for &key in keys {
            let Some(shape) = self.shapes.find(key) else { continue };
            let found = match self.outermost(shape.group) {
                Some(g) => self.members(g),
                None => vec![key],
            };
            whole.extend(found.into_iter().filter(|k| seen.insert(*k)));
        }
        whole
    }

    /// The groups `group` sits in, from itself out to the outermost.
    fn above(&self, mut group: Option<Key<Group>>) -> Vec<Key<Group>> {
        let mut found = Vec::new();
        while let Some(g) = group.filter(|g| !found.contains(g)) {
            found.push(g);
            group = self.groups.find(g).and_then(|g| g.group);
        }
        found
    }

    fn outermost(&self, group: Option<Key<Group>>) -> Option<Key<Group>> {
        self.above(group).last().copied()
    }

    /// The shapes and groups directly in `group`.
    fn held(&self, group: Key<Group>) -> Vec<Top> {
        let shapes = self.shapes.linked(Shape::group, group).map(|s| Top::Shape(s.id));
        let groups = self.groups.linked(Group::group, group).map(|g| Top::Group(g.id));
        shapes.chain(groups).collect()
    }

    /// Every shape in `group`, however deep.
    fn members(&self, group: Key<Group>) -> Vec<Key<Shape>> {
        let mut found = Vec::new();
        let mut due = vec![group];
        while let Some(g) = due.pop() {
            for top in self.held(g) {
                match top {
                    Top::Shape(s) => found.push(s),
                    Top::Group(g) => due.push(g),
                }
            }
        }
        found
    }

    fn lift(&mut self, top: Top, into: Option<Key<Group>>) {
        let _ = match top {
            Top::Shape(s) => self.shapes.change(s, |s| s.group = into),
            Top::Group(g) => self.groups.change(g, |g| g.group = into),
        };
    }

    /// Takes apart the groups among `groups` left holding fewer than two things, and the groups they were in in turn, since a group of one holds nothing together.
    fn tidy(&mut self, groups: Vec<Key<Group>>) {
        let mut due = groups;
        while let Some(g) = due.pop() {
            let Some(parent) = self.groups.find(g).map(|g| g.group) else { continue };
            let held = self.held(g);
            if held.len() >= 2 {
                continue;
            }
            for top in held {
                self.lift(top, parent);
            }
            let _ = self.groups.remove(g);
            due.extend(parent);
        }
    }

    /// Locks or unlocks shapes. A locked shape can't be moved, resized, written in or drawn in another ink.
    pub fn lock(&mut self, keys: &[Key<Shape>], locked: bool) -> Outcome {
        let mut outcome = Outcome::Nothing;
        for &key in keys {
            if self.shapes.find(key).is_some_and(|s| s.locked != locked) {
                outcome = self.shapes.change(key, |s| s.locked = locked);
            }
        }
        outcome
    }

    /// Draws shapes in another ink, leaving locked ones as they are.
    pub fn restyle(&mut self, keys: &[Key<Shape>], ink: Ink) -> Outcome {
        let mut outcome = Outcome::Nothing;
        for &key in keys {
            if self.shapes.find(key).is_some_and(|s| !s.locked && s.ink != ink) {
                outcome = self.shapes.change(key, |s| s.ink = ink);
            }
        }
        outcome
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
        let mut groups = Vec::new();
        for &key in keys {
            groups.extend(self.shapes.find(key).and_then(|s| s.group));
            if self.shapes.remove(key).changed() {
                outcome = Outcome::Changed;
            }
        }
        self.tidy(groups);
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
