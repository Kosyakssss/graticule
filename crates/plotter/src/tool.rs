use std::collections::HashSet;

use graticule_macros::Facts;
use ledger::{Choose, Key, Outcome, Selection};

use crate::board::{Board, End, Kind, Shape};
use crate::geometry::{Bounds, Point};
use crate::index::Index;
use crate::snap::{Gap, Guide, snap, snap_edges};

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
    /// Draw a frame, which takes in the shapes wholly inside it.
    Frame,
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
    /// Choose `shape` and the rest of its group, or with `adding` add them to the chosen ones or take them away; no shape lets go of them all.
    Choose { shape: Option<Key<Shape>>, adding: bool },
    /// Move shapes by a distance, snapped to the others, with the lines and the equal gaps they snapped to, into the frame they are dropped on or out of any.
    Translate { keys: Vec<Key<Shape>>, dx: f64, dy: f64, guides: Vec<Guide>, gaps: Vec<Gap>, into: Option<Key<Shape>> },
    /// Choose the shapes wholly inside a box, with the whole of any group they are in inside it too, which are `keys`, with `adding` as well as the chosen ones.
    Brush { area: Bounds, keys: Vec<Key<Shape>>, adding: bool },
    /// Stretch the chosen shapes from the box around them, `from`, to a new box, by dragging one of its handles: a corner, or the middle of a side, which moves only that side. The dragged edges snap to the other shapes, along `guides`.
    Resize { keys: Vec<Key<Shape>>, from: Bounds, to: Bounds, guides: Vec<Guide> },
    /// Put a new shape on the board, in the frame it is drawn in, hold an arrow's ends to the shapes they start and finish on, and put the shapes a new frame surrounds, `holds`, in it.
    Create { shape: Shape, start: Option<Key<Shape>>, end: Option<Key<Shape>>, holds: Vec<Key<Shape>> },
}

/// How close, in screen points, the pointer must be to a line or a handle to take it, and how close an edge must come to snap.
const REACH: f64 = 6.0;
const SNAP: f64 = 8.0;
/// How far along a row or a column, in screen points, a moved shape looks for others to space itself evenly with.
const ROW: f64 = 1_200.0;
const NEW: f64 = 100.0;

impl Intent {
    /// What `gesture` does with `tool` in hand, on a board seen at `zoom`. Shapes are found through `index`, so it costs what is near the gesture, not what the board holds.
    pub fn of(board: &Board, index: &Index, chosen: &Selection<Shape>, tool: Tool, gesture: &Gesture, zoom: f64) -> Intent {
        let reach = REACH / zoom;
        let (from, to) = (gesture.from, gesture.to);
        match tool {
            Tool::Select => {
                if let Some(Grip { keys, around: b, at: (fx, fy) }) = handle(board, chosen, from, reach) {
                    let mut guides = Vec::new();
                    let mut to = to;
                    if gesture.moved {
                        let mine: HashSet<Key<Shape>> = board.contents(&keys).into_iter().collect();
                        let (near, row) = (SNAP / zoom, ROW / zoom);
                        let mut found = index.query(Bounds::new(to.x - row, to.y - near, 2.0 * row, 2.0 * near));
                        found.extend(index.query(Bounds::new(to.x - near, to.y - row, 2.0 * near, 2.0 * row)));
                        found.sort_unstable_by_key(|e| e.key);
                        found.dedup_by_key(|e| e.key);
                        let others: Vec<Bounds> = found.into_iter().filter(|e| !e.arrow && !mine.contains(&e.key)).map(|e| e.bounds).collect();
                        let snapped = snap_edges((fx != 0.5).then_some(to.x), (fy != 0.5).then_some(to.y), &others, near);
                        to = Point::new(to.x + snapped.dx, to.y + snapped.dy);
                        guides = snapped.guides;
                    }
                    let span = |f: f64, start: f64, size: f64, at: f64| match f {
                        0.5 => (start, start + size),
                        _ => {
                            let fixed = start + (1.0 - f) * size;
                            (fixed.min(at), fixed.max(at))
                        }
                    };
                    let ((x0, x1), (y0, y1)) = (span(fx, b.x, b.w, to.x), span(fy, b.y, b.h, to.y));
                    return match gesture.moved {
                        true => Intent::Resize { keys, from: b, to: Bounds::new(x0, y0, x1 - x0, y1 - y0), guides },
                        false => Intent::Choose { shape: keys.first().copied(), adding: false },
                    };
                }
                match (index.hit(board, from, reach), gesture.moved) {
                    (Some(key), false) => Intent::Choose { shape: Some(key), adding: gesture.shift },
                    (Some(key), true) => {
                        let keys: Vec<_> = if chosen.contains(key) { chosen.iter().filter(|k| board.shapes.has(*k)).collect() } else { board.whole(&[key]) };
                        let (dx, dy) = (to.x - from.x, to.y - from.y);
                        let moving = keys.iter().filter_map(|k| board.shapes.find(*k)).map(|s| s.bounds()).reduce(|a, b| a.union(&b)).unwrap_or_default();
                        let moved = Bounds { x: moving.x + dx, y: moving.y + dy, ..moving };
                        let mine: HashSet<Key<Shape>> = board.contents(&keys).into_iter().collect();
                        let (near, row) = (SNAP / zoom, ROW / zoom);
                        let mut found = index.query(Bounds::new(moved.x - row, moved.y - near, moved.w + 2.0 * row, moved.h + 2.0 * near));
                        found.extend(index.query(Bounds::new(moved.x - near, moved.y - row, moved.w + 2.0 * near, moved.h + 2.0 * row)));
                        found.sort_unstable_by_key(|e| e.key);
                        found.dedup_by_key(|e| e.key);
                        let others: Vec<Bounds> = found.into_iter().filter(|e| !e.arrow && !mine.contains(&e.key)).filter_map(|e| board.shapes.find(e.key)).map(Shape::bounds).collect();
                        let snapped = snap(moved, &others, near);
                        let into = frame_at(board, index, to, &mine);
                        Intent::Translate { keys, dx: dx + snapped.dx, dy: dy + snapped.dy, guides: snapped.guides, gaps: snapped.gaps, into }
                    }
                    (None, false) => Intent::Choose { shape: None, adding: gesture.shift },
                    (None, true) => {
                        let area = Bounds::around(from, to);
                        let found: Vec<Key<Shape>> = index.query(area).into_iter().filter_map(|e| board.shapes.find(e.key)).filter(|s| area.holds(&s.bounds())).map(|s| s.id).collect();
                        let held: HashSet<Key<Shape>> = found.iter().copied().collect();
                        let keys = found.into_iter().filter(|k| board.whole(&[*k]).iter().all(|w| held.contains(w))).collect();
                        Intent::Brush { area, keys, adding: gesture.shift }
                    }
                }
            }
            Tool::Rect | Tool::Ellipse | Tool::Frame => {
                let kind = match tool {
                    Tool::Rect => Kind::Rect,
                    Tool::Ellipse => Kind::Ellipse,
                    _ => Kind::Frame(String::new()),
                };
                let area = match gesture.moved {
                    true => Bounds::around(from, to),
                    false => Bounds::new(from.x - NEW / 2.0, from.y - NEW / 2.0, NEW, NEW),
                };
                let shape = framed(board, index, Shape::new(Key::default(), kind, area));
                let holds = match tool {
                    Tool::Frame => index.query(area).into_iter().filter_map(|e| board.shapes.find(e.key)).filter(|s| s.frame == shape.frame && area.holds(&s.bounds())).map(|s| s.id).collect(),
                    _ => Vec::new(),
                };
                Intent::Create { shape, start: None, end: None, holds }
            }
            Tool::Arrow => {
                let to = if gesture.moved { to } else { from + Point::new(NEW, 0.0) };
                let start = target(board, index, from, reach);
                let end = target(board, index, to, reach).filter(|e| gesture.moved && Some(*e) != start);
                let edge = |held: Option<Key<Shape>>, outside: Point, inside: Point| held.and_then(|k| board.shapes.find(k)).and_then(|s| s.geometry().crossing(outside, inside)).unwrap_or(inside);
                Intent::Create { shape: framed(board, index, Shape::arrow(Key::default(), edge(start, to, from), edge(end, from, to))), start, end, holds: Vec::new() }
            }
            Tool::Draw => {
                let trail = if gesture.trail.is_empty() { vec![from] } else { gesture.trail.clone() };
                let area = trail.iter().fold(Bounds::new(from.x, from.y, 0.0, 0.0), |b, p| b.union(&Bounds::new(p.x, p.y, 0.0, 0.0)));
                let points = trail.iter().flat_map(|p| [p.x - area.x, p.y - area.y]).collect();
                Intent::Create { shape: framed(board, index, Shape::new(Key::default(), Kind::Draw(points), area)), start: None, end: None, holds: Vec::new() }
            }
        }
    }

    /// The shapes a gesture moves or makes: what the board would hold for them if it were let go now. Showing them over the board shows the drag under way without changing the facts.
    pub fn preview(&self, board: &Board) -> Vec<Shape> {
        let keys: Vec<Key<Shape>> = match self {
            Intent::Translate { keys, .. } => board.contents(keys),
            Intent::Resize { keys, .. } => keys.clone(),
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
            Intent::Choose { shape: Some(k), adding } => {
                let whole = self.whole(&[k]);
                let keys: Vec<u128> = match (adding, chosen.contains(k)) {
                    (false, _) => whole.iter().map(|k| k.raw()).collect(),
                    (true, false) => chosen.iter().chain(whole).map(Key::raw).collect(),
                    (true, true) => chosen.iter().filter(|c| !whole.contains(c)).map(Key::raw).collect(),
                };
                chosen.choose(&if keys.is_empty() { Choose::Clear } else { Choose::All(keys) })
            }
            Intent::Choose { shape: None, adding: false } => chosen.choose(&Choose::Clear),
            Intent::Choose { shape: None, adding: true } => Outcome::Nothing,
            Intent::Translate { keys, dx, dy, into, .. } => {
                if keys.iter().any(|k| !chosen.contains(*k)) {
                    let _ = chosen.choose(&Choose::All(keys.iter().map(|k| k.raw()).collect()));
                }
                let moved = self.translate(&keys, dx, dy);
                let carried = self.carried(&keys, |s| !s.locked);
                match self.reframe(&carried, into) {
                    Outcome::Nothing => moved,
                    changed => changed,
                }
            }
            Intent::Brush { keys, adding, .. } => {
                let mut keys: Vec<u128> = keys.into_iter().filter(|k| self.shapes.has(*k)).map(Key::raw).collect();
                if adding {
                    keys.extend(chosen.iter().map(Key::raw));
                }
                chosen.choose(&if keys.is_empty() { Choose::Clear } else { Choose::All(keys) })
            }
            Intent::Resize { keys, from, to, .. } => self.scale(&keys, from, to),
            Intent::Create { mut shape, start, end, holds } => {
                shape.id = Key::from_raw(key());
                let id = shape.id;
                let (from, to) = (shape.end(End::Start), shape.end(End::End));
                let outcome = if shape.index.is_empty() { self.add(shape) } else { self.shapes.push(shape) };
                for (held, end, at) in [(start, End::Start, from), (end, End::End, to)] {
                    if let Some(target) = held {
                        let _ = self.bind(Key::from_raw(key()), id, end, target, at);
                    }
                }
                let _ = self.reframe(&holds, Some(id));
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

/// Whether the box around `shapes` has handles to drag: one shape has them unless it is locked, an arrow or text, whose size its words decide; several have them if any is unlocked.
pub fn resizable<'s>(shapes: impl IntoIterator<Item = &'s Shape>) -> bool {
    let shapes: Vec<&Shape> = shapes.into_iter().collect();
    match shapes.as_slice() {
        [] => false,
        [one] => !one.locked && !matches!(one.kind, Kind::Arrow | Kind::Text(_)),
        many => many.iter().any(|s| !s.locked),
    }
}

/// A handle taken: the shapes it resizes, the box around them and where on it the handle is, as in [`HANDLES`].
struct Grip {
    keys: Vec<Key<Shape>>,
    around: Bounds,
    at: (f64, f64),
}

fn handle(board: &Board, chosen: &Selection<Shape>, p: Point, reach: f64) -> Option<Grip> {
    let shapes: Vec<&Shape> = chosen.iter().filter_map(|k| board.shapes.find(k)).collect();
    if !resizable(shapes.iter().copied()) {
        return None;
    }
    let b = shapes.iter().map(|s| s.bounds()).reduce(|a, b| a.union(&b))?;
    let near = HANDLES.into_iter().find(|&(fx, fy)| Point::new(b.x + fx * b.w, b.y + fy * b.h).distance(p) <= reach)?;
    Some(Grip { keys: shapes.iter().map(|s| s.id).collect(), around: b, at: near })
}

/// A new shape on top of the stack, in the topmost frame under its middle.
fn framed(board: &Board, index: &Index, shape: Shape) -> Shape {
    let frame = frame_at(board, index, shape.bounds().center(), &HashSet::new());
    Shape { frame, ..index.above(shape) }
}

/// The topmost frame whose box holds `p`, other than `except`.
fn frame_at(board: &Board, index: &Index, p: Point, except: &HashSet<Key<Shape>>) -> Option<Key<Shape>> {
    let at = Bounds::new(p.x, p.y, 0.0, 0.0);
    index
        .query(at)
        .into_iter()
        .rev()
        .filter(|e| !e.arrow && !except.contains(&e.key) && e.bounds.contains(p))
        .filter_map(|e| board.shapes.find(e.key))
        .find(|s| matches!(s.kind, Kind::Frame(_)))
        .map(|s| s.id)
}

fn target(board: &Board, index: &Index, p: Point, reach: f64) -> Option<Key<Shape>> {
    let near = Bounds::new(p.x - reach, p.y - reach, reach * 2.0, reach * 2.0);
    index
        .query(near)
        .into_iter()
        .rev()
        .filter(|e| !e.arrow)
        .filter_map(|e| board.shapes.find(e.key))
        .filter(|s| !matches!(s.kind, Kind::Frame(_)))
        .find(|s| s.geometry().inside(p) || s.geometry().hit(p, reach))
        .map(|s| s.id)
}
