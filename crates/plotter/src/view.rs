//! A board on a kernel canvas, for any app built on Plotter: its shapes drawn off the main thread in slices, the box around the chosen ones, and a drag under way shown as what letting go would do. Built with the `kernel` feature.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use kernel::canvas::{self, Weight};
use kernel::image::Image;
use kernel::ui::Canvas;
use kernel::{Camera, Derived, Drag, Drawing, Ink};
use ledger::{Key, Selection};

use crate::board::{Asset, Board, End, Kind, LINE, Page, Shape};
use crate::geometry::{Bounds, Point};
use crate::index::Index;
use crate::tool::{Gesture, HANDLES, Intent, Tool, resizable};

/// How many shapes go in a slice.
const SLICE: usize = 512;

/// How big a board is drawn and indexed on the main thread while the background catches up after a change.
pub const SMALL: usize = 4_096;

/// How many of the shapes in sight the keyboard reaches, from the bottom of the stack, so a screen reader can read them out.
const READ: usize = 512;

/// The most a canvas shows, in points: what is in sight through the camera on the largest screen.
const SIGHT: (f64, f64) = (3_840.0, 2_160.0);

/// The box around the chosen shapes, and whether it has handles to drag, as [`resizable`] says, worked out off the main thread: `#[derived(Around::of, from = [board, chosen])]`.
#[derive(Default)]
pub struct Around(pub Option<(Bounds, bool)>);

impl Around {
    pub fn of(board: &Board, chosen: &Selection<Shape>) -> Around {
        Around::shapes(chosen.iter().filter_map(|k| board.shapes.find(k)))
    }

    /// The box around `shapes`, now.
    pub fn shapes<'s>(shapes: impl Iterator<Item = &'s Shape>) -> Around {
        let shapes: Vec<&Shape> = shapes.collect();
        let around = shapes.iter().map(|s| s.bounds()).reduce(|a, b| a.union(&b));
        Around(around.map(|a| (a, resizable(shapes.iter().copied()))))
    }
}

/// The board's pictures decoded, by asset, off the main thread: `#[derived(Pictures::of, from = [board, pictures])]`. Each is decoded once: working them out again after a change keeps the ones decoded already.
#[derive(Default)]
pub struct Pictures(HashMap<Key<Asset>, Image>);

impl Pictures {
    pub fn of(board: &Board, last: &Derived<Pictures>) -> Pictures {
        Pictures::decode(board, last.get())
    }

    /// Every picture on the board, keeping those `last` decoded already.
    pub fn decode(board: &Board, last: Option<&Pictures>) -> Pictures {
        let decoded = |asset: &Asset| last.and_then(|p| p.0.get(&asset.id).cloned()).unwrap_or_else(|| Image::decode(&asset.bytes()).unwrap_or_default());
        Pictures(board.assets.iter().map(|a| (a.id, decoded(a))).collect())
    }

    /// The picture an image shape shows.
    pub fn get(&self, shape: &Shape) -> Option<&Image> {
        let Kind::Image(asset) = shape.kind else { return None };
        self.0.get(&asset)
    }
}

/// The shapes in sight drawn off the main thread, in slices of the stack, so a drag draws again only the slices holding what it moves: `#[derived(Layers::of, from = [board, index, camera, pictures])]`, or [`Layers::plain`] for a board without pictures.
pub struct Layers {
    slices: Vec<(Vec<Key<Shape>>, Arc<Drawing>)>,
}

impl Layers {
    pub fn of(board: &Board, index: &Derived<Index>, camera: &Camera, pictures: &Derived<Pictures>) -> Layers {
        Layers::drawn(board, index, camera, pictures.get())
    }

    pub fn plain(board: &Board, index: &Derived<Index>, camera: &Camera) -> Layers {
        Layers::drawn(board, index, camera, None)
    }

    fn drawn(board: &Board, index: &Derived<Index>, camera: &Camera, pictures: Option<&Pictures>) -> Layers {
        let Some(index) = index.get() else { return Layers { slices: Vec::new() } };
        let [left, right, top, bottom] = camera.showing(SIGHT.0, SIGHT.1);
        let seen: Vec<&Shape> = index.query(Bounds::new(left, top, right - left, bottom - top)).into_iter().filter_map(|e| board.shapes.find(e.key)).collect();
        let slices = seen
            .chunks(SLICE)
            .map(|shapes| {
                let mut drawing = Drawing::new();
                for shape in shapes {
                    draw(&mut drawing, shape, board.window(shape), pictures);
                }
                drawing.stamp();
                (shapes.iter().map(|s| s.id).collect(), Arc::new(drawing))
            })
            .collect();
        Layers { slices }
    }
}

/// What a board canvas shows: the board and the page on screen, the values worked out off the main thread for it, the chosen shapes, the tool in hand and the camera's zoom.
pub struct Sight<'a> {
    pub board: &'a Board,
    pub page: Key<Page>,
    pub index: &'a Derived<Index>,
    pub layers: &'a Derived<Layers>,
    pub around: &'a Derived<Around>,
    pub pictures: Option<&'a Pictures>,
    pub chosen: &'a Selection<Shape>,
    pub tool: Tool,
    pub zoom: f64,
}

impl<'a> Sight<'a> {
    /// Where the shapes are: the index worked out in the background, or for a small board while that catches up after a change, one made now.
    pub fn index(&self) -> Cow<'a, Index> {
        let fresh = !self.index.coming() || self.board.shapes.len() > SMALL;
        match self.index.get().filter(|_| fresh) {
            Some(index) => Cow::Borrowed(index),
            None if self.board.shapes.len() <= SMALL => Cow::Owned(Index::of(self.board, &self.page)),
            None => Cow::Owned(Index::default()),
        }
    }

    /// What letting go of `drag` would do.
    pub fn intent(&self, drag: &Drag) -> Intent {
        Intent::of(self.board, &self.index(), self.chosen, self.tool, &gesture(drag), self.zoom)
    }

    /// Draws the board on `canvas`, with the drag under way shown as what letting go would do, and `also` drawn in place of the shapes with the same keys, or added. A press on a shape runs `choose` with its key. Give the canvas its camera and its drag command first.
    pub fn show<C, S>(&self, canvas: &mut Canvas<'_, '_, C, S>, also: Vec<Shape>, choose: impl Fn(Key<Shape>) -> C) {
        let board = self.board;
        let intent = canvas.drag().map(|drag| self.intent(drag));
        let mut moved = intent.as_ref().map(|i| i.preview(board)).unwrap_or_default();
        for shape in also {
            if !moved.iter().any(|m| m.id == shape.id) {
                moved.push(shape);
            }
        }
        let moving: HashSet<Key<Shape>> = moved.iter().map(|m| m.id).collect();
        let shown = |key: Key<Shape>| moved.iter().find(|m| m.id == key).or(board.shapes.find(key));
        let window = |shape: &Shape| board.frames(shape.frame).into_iter().filter_map(shown).map(|f| f.bounds()).reduce(|a, b| a.overlap(&b));
        let small = board.shapes.len() <= SMALL;
        let mut reached = Vec::new();
        match self.layers.get().filter(|_| !(small && (self.layers.coming() || self.index.coming()))) {
            Some(layers) => {
                reached.extend(layers.slices.iter().flat_map(|(keys, _)| keys).take(READ).copied());
                for (keys, drawing) in &layers.slices {
                    if keys.iter().any(|k| moving.contains(k)) {
                        let mut redrawn = Drawing::new();
                        for shape in keys.iter().filter_map(|k| shown(*k)) {
                            draw(&mut redrawn, shape, window(shape), self.pictures);
                        }
                        canvas.drawing(&Arc::new(redrawn));
                    } else {
                        canvas.drawing(drawing);
                    }
                }
            }
            None if small => {
                let page = board.showing(self.page);
                reached.extend(board.on(page).iter().take(READ).map(|s| s.id));
                for shape in board.on(page) {
                    let shape = shown(shape.id).unwrap_or(shape);
                    draw(canvas, shape, window(shape), self.pictures);
                }
            }
            None => {}
        }
        for key in reached {
            canvas.press(mark(key), choose(key));
        }
        for shape in moved.iter().filter(|m| !board.shapes.has(m.id)) {
            draw(canvas, shape, window(shape), self.pictures);
        }
        let around = match self.around.get().filter(|_| moved.is_empty() && !self.around.coming()) {
            Some(Around(around)) => *around,
            None => Around::shapes(self.chosen.iter().filter_map(shown)).0,
        };
        if let Some((b, corners)) = around {
            outline(canvas, b, corners);
        }
        match &intent {
            Some(Intent::Brush { area, .. }) => {
                corners(canvas, *area, Ink::Muted);
            }
            Some(Intent::Resize { guides, to, .. }) => {
                let reach = to.expand(400.0);
                for g in guides {
                    let (from, to) = if g.vertical { ((g.at, reach.y), (g.at, reach.bottom())) } else { ((reach.x, g.at), (reach.right(), g.at)) };
                    canvas.line(at(from.0, from.1), at(to.0, to.1)).ink(Ink::Danger).stroke(Weight::Hair);
                }
            }
            Some(Intent::Translate { guides, gaps, .. }) => {
                let reach = moved.iter().map(Shape::bounds).reduce(|a, b| a.union(&b)).unwrap_or_default().expand(400.0);
                for g in guides {
                    let (from, to) = if g.vertical { ((g.at, reach.y), (g.at, reach.bottom())) } else { ((reach.x, g.at), (reach.right(), g.at)) };
                    canvas.line(at(from.0, from.1), at(to.0, to.1)).ink(Ink::Danger).stroke(Weight::Hair);
                }
                let tick = 6.0 / self.zoom;
                for g in gaps {
                    let point = |along: f64, off: f64| if g.across { at(along, g.at + off) } else { at(g.at + off, along) };
                    canvas.line(point(g.from, 0.0), point(g.to, 0.0)).ink(Ink::Danger).stroke(Weight::Hair);
                    for end in [g.from, g.to] {
                        canvas.line(point(end, -tick), point(end, tick)).ink(Ink::Danger).stroke(Weight::Hair);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The key a shape's mark has on the canvas, so the keyboard's ring finds it.
pub fn mark(key: Key<Shape>) -> u64 {
    let raw = key.raw();
    (raw as u64) ^ ((raw >> 64) as u64).rotate_left(32)
}

/// A drag on the canvas as a gesture on the board, both in page units.
pub fn gesture(drag: &Drag) -> Gesture {
    let point = |(x, y): (f64, f64)| Point::new(x, y);
    Gesture { from: point(drag.from), to: point(drag.to), trail: drag.trail.iter().copied().map(point).collect(), moved: drag.moved, shift: drag.shift }
}

/// A point on the board as the canvas takes it.
pub fn at(x: f64, y: f64) -> (f32, f32) {
    (x as f32, y as f32)
}

/// The theme's ink for a shape's ink.
pub fn ink(ink: crate::Ink) -> Ink {
    match ink {
        crate::Ink::Ink => Ink::Ink,
        crate::Ink::Muted => Ink::Muted,
        crate::Ink::Link => Ink::Link,
        crate::Ink::Ok => Ink::Ok,
        crate::Ink::Warn => Ink::Warn,
        crate::Ink::Danger => Ink::Danger,
    }
}

/// Draws one shape, cut to the box of the frames it is in, `window`, an image showing its picture from `pictures`.
pub fn draw(canvas: &mut Drawing, shape: &Shape, window: Option<Bounds>, pictures: Option<&Pictures>) {
    let color = ink(shape.ink);
    let b = Bounds::new(shape.x, shape.y, shape.w, shape.h);
    let edge = window.map(|w| (at(w.x, w.y), at(w.right(), w.bottom())));
    let named = mark(shape.id);
    let fit = |drawn: &mut canvas::Shape| {
        drawn.key(named);
        if let Some((from, to)) = edge {
            drawn.clip(from, to);
        }
    };
    match &shape.kind {
        Kind::Rect => fit(corners(canvas, b, color)),
        Kind::Ellipse => {
            let (c, rx, ry) = (b.center(), b.w / 2.0, b.h / 2.0);
            let k = 0.552_284_75;
            let mark = canvas.path(|pen| {
                pen.move_to(at(c.x + rx, c.y))
                    .curve_to(at(c.x + rx, c.y + ry * k), at(c.x + rx * k, c.y + ry), at(c.x, c.y + ry))
                    .curve_to(at(c.x - rx * k, c.y + ry), at(c.x - rx, c.y + ry * k), at(c.x - rx, c.y))
                    .curve_to(at(c.x - rx, c.y - ry * k), at(c.x - rx * k, c.y - ry), at(c.x, c.y - ry))
                    .curve_to(at(c.x + rx * k, c.y - ry), at(c.x + rx, c.y - ry * k), at(c.x + rx, c.y))
                    .close();
            });
            fit(mark.stroke(Weight::Line).ink(color));
        }
        Kind::Text(text) => {
            let label = canvas.text(at(b.x, b.y + b.h / 2.0), text).ink(color);
            if let Some((from, to)) = edge {
                label.clip(from, to);
            }
        }
        Kind::Draw(_) => fit(canvas.polygon(shape.stroke().unwrap_or_default().iter().map(|p| at(p.x, p.y))).ink(color)),
        Kind::Arrow => {
            let (a, z) = (shape.end(End::Start), shape.end(End::End));
            fit(canvas.line(at(a.x, a.y), at(z.x, z.y)).ink(color));
            let along = z - a;
            let length = along.length();
            if length > 0.0 {
                let (ux, uy) = (along.x / length, along.y / length);
                let head = 12.0_f64.min(length / 2.0);
                let wing = |side: f64| at(z.x - head * ux - side * head * 0.5 * uy, z.y - head * uy + side * head * 0.5 * ux);
                fit(canvas.polyline([wing(1.0), at(z.x, z.y), wing(-1.0)]).ink(color));
            }
        }
        Kind::Image(_) => match pictures.and_then(|p| p.get(shape)).filter(|i| !i.is_empty()) {
            Some(image) => fit(canvas.image(image, at(b.x, b.y), at(b.right(), b.bottom()))),
            None => fit(corners(canvas, b, Ink::Faint).stroke(Weight::Hair)),
        },
        Kind::Frame(name) => {
            let b = shape.bounds();
            fit(corners(canvas, b, color).stroke(Weight::Hair));
            let label = canvas.text(at(b.x, b.y - LINE / 2.0 - 4.0), name).ink(color);
            if let Some((from, to)) = edge {
                label.clip(from, to);
            }
        }
    }
}

/// The edges of a box, as one line around it.
pub fn corners(canvas: &mut Drawing, b: Bounds, color: Ink) -> &mut canvas::Shape {
    canvas.polyline([at(b.x, b.y), at(b.right(), b.y), at(b.right(), b.bottom()), at(b.x, b.bottom()), at(b.x, b.y)]).ink(color)
}

/// The line around chosen shapes, with a dot on each handle when they can be resized.
pub fn outline(canvas: &mut Drawing, b: Bounds, corners: bool) {
    let b = b.expand(4.0);
    canvas.polyline([at(b.x, b.y), at(b.right(), b.y), at(b.right(), b.bottom()), at(b.x, b.bottom()), at(b.x, b.y)]).ink(Ink::Link).stroke(Weight::Hair);
    if corners {
        for (fx, fy) in HANDLES {
            canvas.dot(at(b.x + fx * b.w, b.y + fy * b.h)).ink(Ink::Link);
        }
    }
}
