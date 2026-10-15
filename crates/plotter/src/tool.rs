use std::collections::HashSet;

use graticule_macros::Facts;
use ledger::{Choose, Key, Outcome, Selection};

use crate::board::{Board, End, Kind, Shape};
use crate::geometry::{Bounds, Point};
use crate::index::Index;
use crate::snap::{Guide, snap};

/// What a drag on the board does.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Choose shapes, move them and resize them.
    #[default]
    Select,
    Rect,
    Ellipse,
    Arrow,
    /// Draw freehand.
    Draw,
}

/// A press on the board, dragged and let go, in page units: where it started, where it is now, every place it passed through, whether it went further than a click's wobble and whether Shift was held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gesture {
    pub from: Point,
    pub to: Point,
    pub trail: Vec<Point>,
    pub moved: bool,
    pub shift: bool,
}

/// What letting go of a gesture does, worked out from the board, its index, the chosen shapes and the tool alone. It is the state the tool is in, so showing a drag under way and finishing it can never disagree.
#[derive(Clone, Debug, PartialEq)]
pub enum Intent {
    /// Choose `shape`, or with `adding` add it to the chosen ones or take it away; no shape lets go of them all.
    Choose { shape: Option<Key<Shape>>, adding: bool },
    /// Move shapes by a distance, snapped to the others, with the lines they snapped to.
    Translate { keys: Vec<Key<Shape>>, dx: f64, dy: f64, guides: Vec<Guide> },
    /// Choose the shapes wholly inside a box, which are `keys`, with `adding` as well as the chosen ones.
    Brush { area: Bounds, keys: Vec<Key<Shape>>, adding: bool },
    /// Give a shape a new box by dragging one of its handles: a corner, or the middle of a side, which moves only that side.
    Resize { key: Key<Shape>, to: Bounds },
    /// Put a new shape on the board, and hold an arrow's ends to the shapes they start and finish on.
    Create { shape: Shape, start: Option<Key<Shape>>, end: Option<Key<Shape>> },
}

/// How close, in screen points, the pointer must be to a line or a handle to take it, and how close an edge must come to snap.
const REACH: f64 = 6.0;
const SNAP: f64 = 8.0;
const NEW: f64 = 100.0;

impl Intent {
    /// What `gesture` does with `tool` in hand, on a board seen at `zoom`. Shapes are found through `index`, so it costs what is near the gesture, not what the board holds.
    pub fn of(board: &Board, index: &Index, chosen: &Selection<Shape>, tool: Tool, gesture: &Gesture, zoom: f64) -> Intent {
        let reach = REACH / zoom;
        let (from, to) = (gesture.from, gesture.to);
        match tool {
            Tool::Select => {
                if let Some((key, b, (fx, fy))) = handle(board, chosen, from, reach) {
                    let span = |f: f64, start: f64, size: f64, at: f64| match f {
                        0.5 => (start, start + size),
                        _ => {
                            let fixed = start + (1.0 - f) * size;
                            (fixed.min(at), fixed.max(at))
                        }
                    };
                    let ((x0, x1), (y0, y1)) = (span(fx, b.x, b.w, to.x), span(fy, b.y, b.h, to.y));
                    return match gesture.moved {
                        true => Intent::Resize { key, to: Bounds::new(x0, y0, x1 - x0, y1 - y0) },
                        false => Intent::Choose { shape: Some(key), adding: false },
                    };
                }
                match (index.hit(board, from, reach), gesture.moved) {
                    (Some(key), false) => Intent::Choose { shape: Some(key), adding: gesture.shift },
                    (Some(key), true) => {
                        let keys: Vec<_> = if chosen.contains(key) { chosen.iter().filter(|k| board.shapes.has(*k)).collect() } else { vec![key] };
                        let (dx, dy) = (to.x - from.x, to.y - from.y);
                        let moving = keys.iter().filter_map(|k| board.shapes.find(*k)).map(|s| s.bounds()).reduce(|a, b| a.union(&b)).unwrap_or_default();
                        let moved = Bounds { x: moving.x + dx, y: moving.y + dy, ..moving };
                        let mine: HashSet<Key<Shape>> = keys.iter().copied().collect();
                        let others: Vec<Bounds> =
                            index.query(moved.expand(SNAP / zoom)).into_iter().filter(|e| !e.arrow && !mine.contains(&e.key)).filter_map(|e| board.shapes.find(e.key)).map(Shape::bounds).collect();
                        let snapped = snap(moved, &others, SNAP / zoom);
                        Intent::Translate { keys, dx: dx + snapped.dx, dy: dy + snapped.dy, guides: snapped.guides }
                    }
                    (None, false) => Intent::Choose { shape: None, adding: gesture.shift },
                    (None, true) => {
                        let area = Bounds::around(from, to);
                        let keys = index.query(area).into_iter().filter_map(|e| board.shapes.find(e.key)).filter(|s| area.holds(&s.bounds())).map(|s| s.id).collect();
                        Intent::Brush { area, keys, adding: gesture.shift }
                    }
                }
            }
            Tool::Rect | Tool::Ellipse => {
                let kind = if tool == Tool::Rect { Kind::Rect } else { Kind::Ellipse };
                let area = match gesture.moved {
                    true => Bounds::around(from, to),
                    false => Bounds::new(from.x - NEW / 2.0, from.y - NEW / 2.0, NEW, NEW),
                };
                Intent::Create { shape: index.above(Shape::new(Key::default(), kind, area)), start: None, end: None }
            }
            Tool::Arrow => {
                let to = if gesture.moved { to } else { from + Point::new(NEW, 0.0) };
                let start = target(board, index, from, reach);
                let end = target(board, index, to, reach).filter(|e| gesture.moved && Some(*e) != start);
                let edge = |held: Option<Key<Shape>>, outside: Point, inside: Point| held.and_then(|k| board.shapes.find(k)).and_then(|s| s.geometry().crossing(outside, inside)).unwrap_or(inside);
                Intent::Create { shape: index.above(Shape::arrow(Key::default(), edge(start, to, from), edge(end, from, to))), start, end }
            }
            Tool::Draw => {
                let trail = if gesture.trail.is_empty() { vec![from] } else { gesture.trail.clone() };
                let area = trail.iter().fold(Bounds::new(from.x, from.y, 0.0, 0.0), |b, p| b.union(&Bounds::new(p.x, p.y, 0.0, 0.0)));
                let points = trail.iter().flat_map(|p| [p.x - area.x, p.y - area.y]).collect();
                Intent::Create { shape: index.above(Shape::new(Key::default(), Kind::Draw(points), area)), start: None, end: None }
            }
        }
    }

    /// The shapes a gesture moves or makes: what the board would hold for them if it were let go now. Showing them over the board shows the drag under way without changing the facts.
    pub fn preview(&self, board: &Board) -> Vec<Shape> {
        let keys: Vec<Key<Shape>> = match self {
            Intent::Translate { keys, .. } => keys.clone(),
            Intent::Resize { key, .. } => vec![*key],
            Intent::Create { start, end, .. } => start.iter().chain(end).copied().collect(),
            Intent::Choose { .. } | Intent::Brush { .. } => return Vec::new(),
        };
        let mut part = board.part(&keys);
        let mut chosen = Selection::new();
        let mut n = u64::MAX;
        let _ = part.perform(&mut chosen, self.clone(), || {
            n -= 1;
            Key::<Shape>::fixed(n).raw()
        });
        part.shapes.iter().filter(|s| !board.shapes.find(s.id).is_some_and(|b| b.geometry() == s.geometry())).cloned().collect()
    }
}

impl Board {
    /// Does what an intent says, as one change, choosing what it makes. `key` hands out new keys.
    pub fn perform(&mut self, chosen: &mut Selection<Shape>, intent: Intent, mut key: impl FnMut() -> u128) -> Outcome {
        match intent {
            Intent::Choose { shape: Some(k), adding } => chosen.choose(&if adding { Choose::Toggle(k.raw()) } else { Choose::Only(k.raw()) }),
            Intent::Choose { shape: None, adding: false } => chosen.choose(&Choose::Clear),
            Intent::Choose { shape: None, adding: true } => Outcome::Nothing,
            Intent::Translate { keys, dx, dy, .. } => {
                if keys.iter().any(|k| !chosen.contains(*k)) {
                    let _ = chosen.choose(&Choose::All(keys.iter().map(|k| k.raw()).collect()));
                }
                self.translate(&keys, dx, dy)
            }
            Intent::Brush { keys, adding, .. } => {
                let mut keys: Vec<u128> = keys.into_iter().filter(|k| self.shapes.has(*k)).map(Key::raw).collect();
                if adding {
                    keys.extend(chosen.iter().map(Key::raw));
                }
                chosen.choose(&if keys.is_empty() { Choose::Clear } else { Choose::All(keys) })
            }
            Intent::Resize { key, to } => self.resize(key, to),
            Intent::Create { mut shape, start, end } => {
                shape.id = Key::from_raw(key());
                let id = shape.id;
                let (from, to) = (shape.end(End::Start), shape.end(End::End));
                let outcome = if shape.index.is_empty() { self.add(shape) } else { self.shapes.push(shape) };
                for (held, end, at) in [(start, End::Start, from), (end, End::End, to)] {
                    if let Some(target) = held {
                        let _ = self.bind(Key::from_raw(key()), id, end, target, at);
                    }
                }
                let _ = chosen.choose(&Choose::Only(id.raw()));
                outcome
            }
        }
    }

    /// A board of only `keys`, the arrows held to them and what those arrows are held to, with their bindings: enough to work out how a change to `keys` would move the rest.
    fn part(&self, keys: &[Key<Shape>]) -> Board {
        let touched: HashSet<Key<Shape>> = keys.iter().copied().collect();
        let held: HashSet<Key<Shape>> = self.touching(keys).into_iter().filter_map(|b| self.bindings.find(b)).map(|b| b.arrow).collect();
        let arrows: Vec<Key<Shape>> = held.iter().copied().collect();
        let bindings: Vec<_> = self.touching(&arrows).into_iter().filter_map(|b| self.bindings.find(b)).filter(|b| held.contains(&b.arrow)).cloned().collect();
        let mut inside: Vec<Key<Shape>> = touched.iter().chain(&held).copied().chain(bindings.iter().map(|b| b.to)).collect();
        inside.sort_unstable();
        inside.dedup();
        let mut part = Board::default();
        for s in inside.into_iter().filter_map(|k| self.shapes.find(k)) {
            let _ = part.shapes.push(s.clone());
        }
        for b in bindings {
            let _ = part.bindings.push(b);
        }
        part
    }
}

/// Where a shape's handles are, as fractions of its box across and down: the corners and the middle of each side.
pub const HANDLES: [(f64, f64); 8] = [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0), (0.0, 0.5)];

fn handle(board: &Board, chosen: &Selection<Shape>, p: Point, reach: f64) -> Option<(Key<Shape>, Bounds, (f64, f64))> {
    let mut keys = chosen.iter();
    let (key, None) = (keys.next()?, keys.next()) else { return None };
    let shape = board.shapes.find(key).filter(|s| s.kind != Kind::Arrow && !s.locked)?;
    let b = shape.bounds();
    let near = HANDLES.into_iter().find(|&(fx, fy)| Point::new(b.x + fx * b.w, b.y + fy * b.h).distance(p) <= reach)?;
    Some((key, b, near))
}

fn target(board: &Board, index: &Index, p: Point, reach: f64) -> Option<Key<Shape>> {
    let near = Bounds::new(p.x - reach, p.y - reach, reach * 2.0, reach * 2.0);
    index.query(near).into_iter().rev().filter(|e| !e.arrow).filter_map(|e| board.shapes.find(e.key)).find(|s| s.geometry().inside(p) || s.geometry().hit(p, reach)).map(|s| s.id)
}
