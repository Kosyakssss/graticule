//! Canvases: boxes on the grid with a coordinate space of their own, holding shapes drawn at runtime in the theme's inks and the style's weights.

use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::{Arc, OnceLock};

use parley::{FontFamily, LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Cap, Command, Format, HitTest, Join, Mask, PathBuilder, Placement, Point, Scratch, Stroke, Vector};

use ledger::{Field, Value};

use crate::ink::{Color, Ink, Theme};

/// Where a canvas looks on an endless page: the page point at its top left corner, and how many of the screen's points one page unit takes. Keep it in the facts, as `#[session] camera: Camera`, and give it to `Canvas::camera`: what the canvas draws is then in page units, the wheel pans it and, with the primary key held, zooms around the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

impl Camera {
    /// The least and most a camera zooms.
    pub const ZOOMS: (f64, f64) = (0.05, 16.0);

    /// The page point under a point of the canvas, in points from its top left.
    pub fn to_page(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (self.x + x / self.zoom, self.y + y / self.zoom)
    }

    /// Where a page point shows on the canvas, in points from its top left.
    pub fn to_screen(&self, (x, y): (f64, f64)) -> (f64, f64) {
        ((x - self.x) * self.zoom, (y - self.y) * self.zoom)
    }

    /// The camera moved by a distance in points, as a drag or the wheel pans it.
    pub fn panned(&self, dx: f64, dy: f64) -> Camera {
        Camera { x: self.x - dx / self.zoom, y: self.y - dy / self.zoom, ..*self }
    }

    /// The camera zoomed by `factor` around a point of the canvas, which stays over the same page point.
    pub fn zoomed(&self, factor: f64, around: (f64, f64)) -> Camera {
        let zoom = (self.zoom * factor).clamp(Self::ZOOMS.0, Self::ZOOMS.1);
        let page = self.to_page(around);
        Camera { x: page.0 - around.0 / zoom, y: page.1 - around.1 / zoom, zoom }
    }

    /// The part of the page a canvas `width` by `height` points shows: left, right, top and bottom.
    pub fn showing(&self, width: f64, height: f64) -> [f64; 4] {
        [self.x, self.x + width / self.zoom, self.y, self.y + height / self.zoom]
    }
}

impl Field for Camera {
    ledger::register!(Camera);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::List(vec![Value::Float(self.x), Value::Float(self.y), Value::Float(self.zoom)])
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::List(parts) => match parts.as_slice() {
                [Value::Float(x), Value::Float(y), Value::Float(zoom)] => Some(Camera { x: *x, y: *y, zoom: zoom.clamp(Self::ZOOMS.0, Self::ZOOMS.1) }),
                _ => None,
            },
            _ => None,
        }
    }
}
impl ledger::Document for Camera {}

/// A press on a canvas, dragged and let go, in the canvas's own space (the page, when it has a camera). `Canvas::on_drag` runs a command with it when it is let go, and `Canvas::drag` gives the one under way, for the view to show what letting go would do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drag {
    /// Where it was pressed.
    pub from: (f64, f64),
    /// Where it is now, or was let go.
    pub to: (f64, f64),
    /// Every place it passed through, from the press on.
    pub trail: Vec<(f64, f64)>,
    /// Whether it went further than a click's wobble.
    pub moved: bool,
    /// Whether Shift was held when it was pressed.
    pub shift: bool,
}

impl Field for Drag {
    ledger::register!(Drag);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        let point = |(x, y): (f64, f64)| Value::List(vec![Value::Float(x), Value::Float(y)]);
        Value::Map(vec![
            ("from".into(), point(self.from)),
            ("to".into(), point(self.to)),
            ("trail".into(), Value::List(self.trail.iter().copied().map(point).collect())),
            ("moved".into(), Value::Bool(self.moved)),
            ("shift".into(), Value::Bool(self.shift)),
        ])
    }
    fn from_value(value: &Value) -> Option<Self> {
        let point = |v: &Value| match v {
            Value::List(p) => match p.as_slice() {
                [Value::Float(x), Value::Float(y)] => Some((*x, *y)),
                _ => None,
            },
            _ => None,
        };
        let Value::Map(fields) = value else { return None };
        let get = |name: &str| fields.iter().find(|(k, _)| k == name).map(|(_, v)| v);
        let trail = match get("trail")? {
            Value::List(points) => points.iter().map(point).collect::<Option<_>>()?,
            _ => return None,
        };
        let flag = |name: &str| matches!(get(name), Some(Value::Bool(true)));
        Some(Drag { from: point(get("from")?)?, to: point(get("to")?)?, trail, moved: flag("moved"), shift: flag("shift") })
    }
}
impl ledger::Document for Drag {}

/// How heavy a line on a canvas is, in the style's weights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weight {
    /// The thinnest line the screen shows.
    Hair,
    /// The style's line, as rules and borders are drawn.
    #[default]
    Line,
    /// Twice the style's line.
    Bold,
}

/// How a shape is drawn: filled, or its outline stroked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Fill,
    Stroke(Weight),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Form {
    Path(u32, u32),
    Rect(Point, Point),
    Dot(Point),
}

/// A shape on a canvas: filled or stroked in one ink, and named by a key that hit tests and presses use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    key: u64,
    ink: Ink,
    mark: Mark,
    form: Form,
    clip: Option<[Point; 2]>,
}

impl Shape {
    /// Draws it in `ink`.
    pub fn ink(&mut self, ink: Ink) -> &mut Self {
        self.ink = ink;
        self
    }

    /// Strokes its outline in `weight`.
    pub fn stroke(&mut self, weight: Weight) -> &mut Self {
        self.mark = Mark::Stroke(weight);
        self
    }

    /// Fills it.
    pub fn fill(&mut self) -> &mut Self {
        self.mark = Mark::Fill;
        self
    }

    /// Names it `key`, for `Canvas::press` and hit tests. Until it is named, a shape's key is its place in its drawing, counting from 0.
    pub fn key(&mut self, key: u64) -> &mut Self {
        self.key = key;
        self
    }

    /// Shows only the part of it inside the box from `from` to `to`, in the drawing's space, as a frame hides what reaches past its edge. Only that part is hit.
    pub fn clip(&mut self, from: (f32, f32), to: (f32, f32)) -> &mut Self {
        self.clip = Some([from.into(), to.into()]);
        self
    }
}

/// Words on a canvas, starting at a point and centred on it from top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    at: Point,
    text: Range<u32>,
    ink: Ink,
    clip: Option<[Point; 2]>,
}

impl Label {
    /// Writes it in `ink`.
    pub fn ink(&mut self, ink: Ink) -> &mut Self {
        self.ink = ink;
        self
    }

    /// Shows only the part of it inside the box from `from` to `to`, in the drawing's space.
    pub fn clip(&mut self, from: (f32, f32), to: (f32, f32)) -> &mut Self {
        self.clip = Some([from.into(), to.into()]);
        self
    }
}

/// A pen for a path of its own, in the drawing's space.
pub struct Pen<'d> {
    commands: &'d mut Vec<Command>,
}

impl Pen<'_> {
    /// Starts a new part of the path at `to`.
    pub fn move_to(&mut self, to: (f32, f32)) -> &mut Self {
        self.commands.move_to(to);
        self
    }

    /// A straight line to `to`.
    pub fn line_to(&mut self, to: (f32, f32)) -> &mut Self {
        self.commands.line_to(to);
        self
    }

    /// A curve to `to`, pulled toward `control`.
    pub fn quad_to(&mut self, control: (f32, f32), to: (f32, f32)) -> &mut Self {
        self.commands.quad_to(control, to);
        self
    }

    /// A curve to `to`, leaving toward `first` and arriving from `second`.
    pub fn curve_to(&mut self, first: (f32, f32), second: (f32, f32), to: (f32, f32)) -> &mut Self {
        self.commands.curve_to(first, second, to);
        self
    }

    /// Closes the part of the path begun last.
    pub fn close(&mut self) -> &mut Self {
        self.commands.close();
        self
    }
}

/// Shapes and words in a space of the app's own: what a canvas shows. Build one in the view through `Canvas`, or in a job when there are many shapes and hand it to `Canvas::drawing`.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    x: (f32, f32),
    y: (f32, f32),
    shapes: Vec<Shape>,
    commands: Vec<Command>,
    labels: Vec<Label>,
    text: String,
    stamp: OnceLock<u64>,
}

impl Default for Drawing {
    fn default() -> Self {
        Self { x: (0.0, 1.0), y: (0.0, 1.0), shapes: Vec::new(), commands: Vec::new(), labels: Vec::new(), text: String::new(), stamp: OnceLock::new() }
    }
}

/// The sizes a drawing is drawn at: the box in pixels, the style's weights and a dot's radius, and the part of its space the box shows when a camera decides it rather than the drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub width: u32,
    pub height: u32,
    pub weights: [f32; 3],
    pub dot: f32,
    pub space: Option<[f32; 4]>,
}

impl Scale {
    fn weight(&self, w: Weight) -> f32 {
        self.weights[w as usize]
    }

    fn hash(&self, h: &mut impl Hasher) {
        (self.width, self.height, self.dot.to_bits(), self.space.map(|s| s.map(f32::to_bits))).hash(h);
        for w in self.weights {
            w.to_bits().hash(h);
        }
    }
}

impl Drawing {
    /// An empty drawing whose space runs from 0 to 1 across and down.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets its space: `x` runs from the left edge to the right, `y` from the top edge to the bottom. Give `y` from high to low for an axis that grows upward, as charts do.
    pub fn space(&mut self, x: Range<f32>, y: Range<f32>) -> &mut Self {
        self.stamp = OnceLock::new();
        (self.x, self.y) = ((x.start, x.end), (y.start, y.end));
        self
    }

    fn push(&mut self, mark: Mark, form: Form) -> &mut Shape {
        self.stamp = OnceLock::new();
        let key = self.shapes.len() as u64;
        self.shapes.push(Shape { key, ink: Ink::Ink, mark, form, clip: None });
        let last = self.shapes.len() - 1;
        &mut self.shapes[last]
    }

    fn open(&mut self, points: impl IntoIterator<Item = (f32, f32)>, close: bool) -> (u32, u32) {
        let start = self.commands.len() as u32;
        for (k, p) in points.into_iter().enumerate() {
            if k == 0 {
                self.commands.move_to(p);
            } else {
                self.commands.line_to(p);
            }
        }
        if close && self.commands.len() as u32 > start {
            self.commands.close();
        }
        (start, self.commands.len() as u32)
    }

    /// A straight line, stroked in the style's line.
    pub fn line(&mut self, from: (f32, f32), to: (f32, f32)) -> &mut Shape {
        let (a, b) = self.open([from, to], false);
        self.push(Mark::Stroke(Weight::Line), Form::Path(a, b))
    }

    /// Straight lines through `points` in turn, stroked in the style's line: a series on a chart.
    pub fn polyline(&mut self, points: impl IntoIterator<Item = (f32, f32)>) -> &mut Shape {
        let (a, b) = self.open(points, false);
        self.push(Mark::Stroke(Weight::Line), Form::Path(a, b))
    }

    /// The shape with corners at `points`, filled.
    pub fn polygon(&mut self, points: impl IntoIterator<Item = (f32, f32)>) -> &mut Shape {
        let (a, b) = self.open(points, true);
        self.push(Mark::Fill, Form::Path(a, b))
    }

    /// The rectangle between two corners, filled.
    pub fn rect(&mut self, from: (f32, f32), to: (f32, f32)) -> &mut Shape {
        self.push(Mark::Fill, Form::Rect(from.into(), to.into()))
    }

    /// A round mark at a point, the same size however the space is stretched, filled.
    pub fn dot(&mut self, at: (f32, f32)) -> &mut Shape {
        self.push(Mark::Fill, Form::Dot(at.into()))
    }

    /// A path drawn with a pen, filled.
    pub fn path(&mut self, f: impl FnOnce(&mut Pen)) -> &mut Shape {
        let start = self.commands.len() as u32;
        f(&mut Pen { commands: &mut self.commands });
        let end = self.commands.len() as u32;
        self.push(Mark::Fill, Form::Path(start, end))
    }

    /// Words starting at `at`, in the interface's monospace face.
    pub fn text(&mut self, at: (f32, f32), text: &str) -> &mut Label {
        self.stamp = OnceLock::new();
        let start = self.text.len() as u32;
        self.text.push_str(text);
        self.labels.push(Label { at: at.into(), text: start..self.text.len() as u32, ink: Ink::Muted, clip: None });
        let last = self.labels.len() - 1;
        &mut self.labels[last]
    }

    /// Whether it holds nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.labels.is_empty()
    }

    /// A hash of everything it draws, worked out once: a drawing made in a job can call this before handing it over, so the view never does.
    pub fn stamp(&self) -> u64 {
        *self.stamp.get_or_init(|| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for v in [self.x.0, self.x.1, self.y.0, self.y.1] {
                v.to_bits().hash(&mut h);
            }
            for s in &self.shapes {
                (s.key, s.ink, s.mark, s.clip.map(|c| c.map(|p| (p.x.to_bits(), p.y.to_bits())))).hash(&mut h);
                match s.form {
                    Form::Path(a, b) => (0u8, a, b).hash(&mut h),
                    Form::Rect(a, b) => (1u8, a.x.to_bits(), a.y.to_bits(), b.x.to_bits(), b.y.to_bits()).hash(&mut h),
                    Form::Dot(a) => (2u8, a.x.to_bits(), a.y.to_bits()).hash(&mut h),
                }
            }
            for c in &self.commands {
                let points: &[Point] = match c {
                    Command::MoveTo(p) | Command::LineTo(p) => &[*p],
                    Command::QuadTo(a, b) => &[*a, *b],
                    Command::CurveTo(a, b, c) => &[*a, *b, *c],
                    Command::Close => &[],
                };
                points.len().hash(&mut h);
                for p in points {
                    (p.x.to_bits(), p.y.to_bits()).hash(&mut h);
                }
            }
            for l in &self.labels {
                (l.at.x.to_bits(), l.at.y.to_bits(), l.ink, &self.text[l.text.start as usize..l.text.end as usize], l.clip.map(|c| c.map(|p| (p.x.to_bits(), p.y.to_bits())))).hash(&mut h);
            }
            h.finish()
        })
    }

    /// Its space: left, right, top and bottom.
    pub fn bounds_of_space(&self) -> [f32; 4] {
        [self.x.0, self.x.1, self.y.0, self.y.1]
    }

    /// Where a point of its space lands in a box drawn at `scale`, in pixels from the box's top left.
    pub fn place(&self, (x, y): (f32, f32), scale: &Scale) -> (f32, f32) {
        let [x0, x1, y0, y1] = scale.space.unwrap_or(self.bounds_of_space());
        let across = |v: f32, a: f32, b: f32, size: u32| if b == a { 0.0 } else { (v - a) / (b - a) * size as f32 };
        (across(x, x0, x1, scale.width), across(y, y0, y1, scale.height))
    }

    /// Its words, each with where it starts in a box drawn at `scale`, its ink and the box it is clipped to, in pixels as left, top, right and bottom.
    pub fn labels<'d>(&'d self, scale: &'d Scale) -> impl Iterator<Item = (&'d str, (f32, f32), Ink, Option<[f32; 4]>)> {
        self.labels.iter().map(move |l| (&self.text[l.text.start as usize..l.text.end as usize], self.place((l.at.x, l.at.y), scale), l.ink, self.limit(l.clip, scale)))
    }

    fn limit(&self, clip: Option<[Point; 2]>, scale: &Scale) -> Option<[f32; 4]> {
        let [a, b] = clip?;
        let (a, b) = (self.place((a.x, a.y), scale), self.place((b.x, b.y), scale));
        Some([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)])
    }

    fn outline(&self, s: &Shape, scale: &Scale, out: &mut Vec<Command>) {
        let at = |p: Point| Point::from(self.place((p.x, p.y), scale));
        match s.form {
            Form::Path(a, b) => out.extend(self.commands[a as usize..b as usize].iter().map(|c| match *c {
                Command::MoveTo(p) => Command::MoveTo(at(p)),
                Command::LineTo(p) => Command::LineTo(at(p)),
                Command::QuadTo(c, p) => Command::QuadTo(at(c), at(p)),
                Command::CurveTo(c1, c2, p) => Command::CurveTo(at(c1), at(c2), at(p)),
                Command::Close => Command::Close,
            })),
            Form::Rect(a, b) => {
                let (a, b) = (at(a), at(b));
                out.add_rect((a.x.min(b.x), a.y.min(b.y)), (a.x - b.x).abs(), (a.y - b.y).abs());
            }
            Form::Dot(p) => {
                out.add_circle(at(p), scale.dot);
            }
        }
    }

    /// The key of the topmost of its shapes under `at`, in pixels from the box's top left, drawn at `scale`. A thin line counts a little around it, so it can be hit.
    pub fn hit(&self, scale: &Scale, at: (f32, f32), slop: f32) -> Option<u64> {
        let mut scratch = Scratch::new();
        let mut path = Vec::new();
        self.shapes.iter().rev().filter(|s| self.limit(s.clip, scale).is_none_or(|[x0, y0, x1, y1]| (x0..=x1).contains(&at.0) && (y0..=y1).contains(&at.1))).find_map(|s| {
            path.clear();
            self.outline(s, scale, &mut path);
            let near = |scratch: &mut Scratch, width: f32| HitTest::with_scratch(&path[..], scratch).style(Stroke::new(width)).test(at);
            let hit = match s.mark {
                Mark::Stroke(w) => near(&mut scratch, scale.weight(w) + slop * 2.0),
                Mark::Fill => HitTest::with_scratch(&path[..], &mut scratch).test(at) || near(&mut scratch, slop * 2.0),
            };
            hit.then_some(s.key)
        })
    }

    /// The box around the shape named `key`, in pixels from the box's top left, drawn at `scale`.
    pub fn bounds(&self, scale: &Scale, key: u64) -> Option<[f32; 4]> {
        let s = self.shapes.iter().find(|s| s.key == key)?;
        let mut path = Vec::new();
        self.outline(s, scale, &mut path);
        let b = match s.mark {
            Mark::Stroke(w) => swash::zeno::bounds(&path[..], Stroke::new(scale.weight(w)), None),
            Mark::Fill => swash::zeno::bounds(&path[..], swash::zeno::Fill::NonZero, None),
        };
        let [x0, y0, x1, y1] = self.limit(s.clip, scale).unwrap_or([f32::MIN, f32::MIN, f32::MAX, f32::MAX]);
        let (left, top, right, bottom) = (b.min.x.max(x0), b.min.y.max(y0), b.max.x.min(x1), b.max.y.min(y1));
        Some([left, top, (right - left).max(0.0), (bottom - top).max(0.0)])
    }
}

/// A canvas to draw off the main thread: its drawings, the sizes to draw them at and the theme's inks, for the texture `id`.
#[derive(Clone, Debug)]
pub struct Raster {
    pub id: u32,
    pub scale: Scale,
    pub drawings: Vec<Arc<Drawing>>,
    pub theme: Theme,
}

impl Raster {
    /// A hash of everything that decides its pixels.
    pub fn stamp(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut h);
        self.scale.hash(&mut h);
        for d in &self.drawings {
            d.stamp().hash(&mut h);
        }
        for ink in Ink::ALL {
            for v in self.theme.get(ink) {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish()
    }

    /// Its pixels as a PNG file's bytes, for exporting a picture, with its words set in the bundled monospace face `words` pixels high. It loads its own fonts, so call it from a job or a file's contents, never from the view.
    pub fn png(&self, words: f32) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.scale.width.max(1), self.scale.height.max(1));
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut pixels = self.rgba();
        self.set_words(&mut pixels, words);
        let written = encoder.write_header().and_then(|mut w| w.write_image_data(&pixels));
        if written.is_err() {
            out.clear();
        }
        out
    }

    fn set_words(&self, out: &mut [u8], size: f32) {
        if size <= 0.0 || self.drawings.iter().all(|d| d.labels.is_empty()) {
            return;
        }
        let (w, h) = (self.scale.width as usize, self.scale.height as usize);
        let mut fonts = crate::fonts::Fonts::new(false);
        let mono = fonts.mono.clone();
        let mut layouts = LayoutContext::<u8>::new();
        let mut scale = ScaleContext::new();
        let line = (size * 1.4).round();
        for d in &self.drawings {
            for (words, (x, y), ink, clip) in d.labels(&self.scale) {
                let limit = pixels(clip, (w, h));
                let color = self.theme.get(ink);
                let mut b = layouts.ranged_builder(&mut fonts.cx, words, 1.0, true);
                b.push_default(StyleProperty::FontFamily(FontFamily::named(&mono)));
                b.push_default(StyleProperty::FontSize(size));
                b.push_default(StyleProperty::LineHeight(LineHeight::Absolute(line)));
                let mut layout = b.build(words);
                layout.break_all_lines(None);
                let top = y - line / 2.0;
                for item in layout.lines().flat_map(|l| l.items().collect::<Vec<_>>()) {
                    let PositionedLayoutItem::GlyphRun(run) = item else { continue };
                    let font = run.run().font();
                    let Some(face) = swash::FontRef::from_index(font.data.data(), font.index as usize) else { continue };
                    let mut scaler = scale.builder(face).size(run.run().font_size()).hint(false).normalized_coords(run.run().normalized_coords().iter()).build();
                    for glyph in run.positioned_glyphs() {
                        let (gx, gy) = (x + glyph.x, top + glyph.y);
                        let render =
                            Render::new(&[Source::Outline, Source::Bitmap(StrikeWith::BestFit)]).format(Format::Alpha).offset(Vector::new(gx - gx.floor(), 0.0)).render(&mut scaler, glyph.id as u16);
                        if let Some(image) = render {
                            let at = Placement { left: gx.floor() as i32 + image.placement.left, top: gy.round() as i32 - image.placement.top, ..image.placement };
                            blend(out, (w, h), &image.data, at, color, limit);
                        }
                    }
                }
            }
        }
    }

    /// Its pixels, as RGBA with straight alpha, row by row.
    pub fn rgba(&self) -> Vec<u8> {
        let (w, h) = (self.scale.width as usize, self.scale.height as usize);
        let mut out = vec![0u8; w * h * 4];
        let mut scratch = Scratch::new();
        let mut path = Vec::new();
        for d in &self.drawings {
            for s in &d.shapes {
                let color = self.theme.get(s.ink);
                let limit = pixels(d.limit(s.clip, &self.scale), (w, h));
                if let (Form::Rect(a, b), Mark::Fill) = (s.form, s.mark) {
                    let (a, b) = (d.place((a.x, a.y), &self.scale), d.place((b.x, b.y), &self.scale));
                    let span = |p: f32, q: f32, least: i32, most: i32| (p.min(q).round().clamp(least as f32, most as f32) as usize, p.max(q).round().clamp(least as f32, most as f32) as usize);
                    let ((x0, x1), (y0, y1)) = (span(a.0, b.0, limit[0], limit[2]), span(a.1, b.1, limit[1], limit[3]));
                    for y in y0..y1 {
                        for x in x0..x1 {
                            over(&mut out[(y * w + x) * 4..][..4], color, color[3]);
                        }
                    }
                    continue;
                }
                path.clear();
                d.outline(s, &self.scale, &mut path);
                let mut mask = Mask::with_scratch(&path[..], &mut scratch);
                if let Mark::Stroke(weight) = s.mark {
                    let mut stroke = Stroke::new(self.scale.weight(weight));
                    stroke.join(Join::Round).caps(Cap::Butt, Cap::Butt);
                    mask.style(stroke);
                }
                let (data, at) = mask.render();
                blend(&mut out, (w, h), &data, at, color, limit);
            }
        }
        out
    }
}

/// The pixels a clip box leaves, as left, top, right and bottom, within a picture `w` by `h`.
fn pixels(clip: Option<[f32; 4]>, (w, h): (usize, usize)) -> [i32; 4] {
    let [x0, y0, x1, y1] = clip.unwrap_or([0.0, 0.0, w as f32, h as f32]);
    let fit = |v: f32, most: usize| v.round().clamp(0.0, most as f32) as i32;
    [fit(x0, w), fit(y0, h), fit(x1, w), fit(y1, h)]
}

fn blend(out: &mut [u8], (w, _): (usize, usize), mask: &[u8], at: Placement, color: Color, [x0, y0, x1, y1]: [i32; 4]) {
    for my in 0..at.height as i32 {
        let y = at.top + my;
        if y < y0 || y >= y1 {
            continue;
        }
        for mx in 0..at.width as i32 {
            let x = at.left + mx;
            if x < x0 || x >= x1 {
                continue;
            }
            let cover = mask[(my * at.width as i32 + mx) as usize];
            if cover > 0 {
                over(&mut out[(y as usize * w + x as usize) * 4..][..4], color, color[3] * f32::from(cover) / 255.0);
            }
        }
    }
}

fn over(px: &mut [u8], color: Color, a: f32) {
    let below = f32::from(px[3]) / 255.0;
    let alpha = a + below * (1.0 - a);
    if alpha <= 0.0 {
        return;
    }
    for k in 0..3 {
        let d = f32::from(px[k]) / 255.0;
        px[k] = (((color[k] * a + d * below * (1.0 - a)) / alpha).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    px[3] = (alpha * 255.0).round() as u8;
}

/// A box painted with the app's own GPU commands: the rung of the ladder past shapes, for what shapes can't draw, such as a scan with levels applied. It paints off the main thread into a texture of its own, which the main thread only places.
pub trait Paint: Send + Sync {
    /// A number that changes whenever what it paints does. The box is painted again only when this, its size or the theme changes.
    fn stamp(&self) -> u64;

    /// Paints the whole of `target`.
    fn paint(&self, target: &Target);
}

/// Where a [`Paint`] paints: the GPU, a texture the size of its box, and the theme to follow.
pub struct Target<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub view: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub theme: Theme,
}

/// A [`Paint`] to keep in the facts as a `#[resource]` and hand to `Canvas::paint`.
#[derive(Clone, Default)]
pub struct PaintHook(Option<Arc<dyn Paint>>);

impl PaintHook {
    /// The hook that paints with `paint`.
    pub fn new(paint: impl Paint + 'static) -> Self {
        PaintHook(Some(Arc::new(paint)))
    }

    pub(crate) fn get(&self) -> Option<&Arc<dyn Paint>> {
        self.0.as_ref()
    }
}

impl std::fmt::Debug for PaintHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "PaintHook" } else { "PaintHook(none)" })
    }
}

impl ledger::Field for PaintHook {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), _: ledger::ErasedPath, _: &mut Vec<ledger::Op>) {}

    fn same(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }

    fn value(&self) -> ledger::Value {
        ledger::Value::Unit
    }

    fn from_value(_: &ledger::Value) -> Option<Self> {
        None
    }
}

/// A box to paint off the main thread with a [`Paint`], into the texture `id`.
#[derive(Clone)]
pub struct Hooked {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub paint: Arc<dyn Paint>,
    pub theme: Theme,
}

impl Hooked {
    /// A hash of everything that decides its pixels.
    pub fn stamp(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.id, self.width, self.height, self.paint.stamp()).hash(&mut h);
        for ink in Ink::ALL {
            for v in self.theme.get(ink) {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish()
    }
}
