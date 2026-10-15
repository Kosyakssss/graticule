use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use parley::{Alignment, AlignmentOptions, FontFamily, FontStyle, FontWeight, Layout, LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Format, Vector};

use crate::fonts::{Fonts, Han};
use crate::gpu::{ATLAS, Quad};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Line {
    Up,
    Down,
    Start,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    Mono,
    Prop,
}

#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub face: Face,
    pub size: f32,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpanStyle {
    pub face: Face,
    pub weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub color: u8,
}

impl SpanStyle {
    pub fn plain(face: Face) -> Self {
        Self { face, weight: 400, italic: false, underline: false, color: 0 }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
struct GlyphKey {
    font: u64,
    index: u32,
    glyph: u32,
    size: u32,
    coords: u64,
    sub: u8,
}

type GlyphRun = (parley::FontData, Range<usize>, [f32; 4], Range<usize>);

#[derive(Default)]
struct Scratch {
    runs: Vec<GlyphRun>,
    coords: Vec<i16>,
    glyphs: Vec<(GlyphKey, f32, f32)>,
    rules: Vec<([f32; 4], [f32; 4])>,
}

impl Scratch {
    fn clear(&mut self) {
        self.runs.clear();
        self.coords.clear();
        self.glyphs.clear();
        self.rules.clear();
    }
}

/// How many cells a character takes in mono text: two for Chinese, Japanese and Korean, as in a terminal.
pub fn cells(c: char) -> usize {
    match c as u32 {
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

/// Whether a character belongs to a script whose letters join, combine or run right to left, so text holding it has to be shaped as a whole rather than drawn a cell at a time.
pub fn shaped(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x0590..=0x08FF | 0x0900..=0x0DFF | 0x0E00..=0x0EFF | 0x0F00..=0x0FFF | 0x1000..=0x109F | 0x1780..=0x17FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

#[derive(Clone, Copy)]
struct Slot {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

pub fn mask<D: swash::zeno::PathData>(d: D, o: &Outline) -> (Vec<u8>, swash::zeno::Placement) {
    let mut mask = swash::zeno::Mask::new(d);
    match o.stroke {
        Some(w) => {
            mask.style(swash::zeno::Stroke::new(w));
        }
        None if o.even_odd => {
            mask.style(swash::zeno::Fill::EvenOdd);
        }
        None => {}
    }
    if o.scale != 1.0 {
        mask.transform(Some(swash::zeno::Transform::scale(o.scale, o.scale)));
    }
    mask.render()
}

pub struct Upload {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

struct Shelf {
    x: u32,
    y: u32,
    row: u32,
}

impl Shelf {
    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.x + w + 1 > ATLAS {
            self.x = 0;
            self.y += self.row + 1;
            self.row = 0;
        }
        if self.y + h + 1 > ATLAS {
            return None;
        }
        let at = (self.x, self.y);
        self.x += w + 1;
        self.row = self.row.max(h);
        Some(at)
    }
}

enum Fallback {
    Off,
    Loading(std::sync::mpsc::Receiver<Fonts>),
    Loaded,
}

pub struct Text {
    fonts: Fonts,
    fallback: Fallback,
    wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    layouts_cx: LayoutContext<u8>,
    scale: ScaleContext,
    layouts: HashMap<u64, (Layout<u8>, String, u64)>,
    glyphs: HashMap<GlyphKey, Option<Slot>>,
    shapes: HashMap<u64, Option<Slot>>,
    shelf: Shelf,
    pub uploads: Vec<Upload>,
    frame: u64,
    scratch: Scratch,
    pub(crate) pen: Vec<crate::px::Rect>,
    pub(crate) atlas: u64,
    /// The canvases to draw off the main thread before the next frame is shown.
    pub rasters: Vec<crate::canvas::Raster>,
    /// The canvas textures no longer on screen, to let go of.
    pub released: Vec<u32>,
    /// The images to send to the GPU before the next frame is shown.
    pub images: Vec<crate::image::Image>,
    /// The paint hooks to run before the next frame is shown.
    pub hooks: Vec<crate::canvas::Hooked>,
    drawn: HashMap<u32, u64>,
}

pub struct Drawn {
    pub key: u64,
    pub width: f32,
    pub height: f32,
    pub underlines: Vec<([f32; 4], Range<usize>)>,
}

#[derive(Clone, Copy)]
pub struct Block<'a> {
    pub spans: &'a [(&'a str, SpanStyle)],
    pub size: f32,
    pub line: f32,
    pub wrap: Option<f32>,
}

#[derive(Clone, Copy)]
pub enum Data<'a> {
    Svg(&'a str),
    Commands(&'a [swash::zeno::Command]),
}

pub struct Outline<'a> {
    pub path: Data<'a>,
    pub stroke: Option<f32>,
    pub even_odd: bool,
    pub scale: f32,
}

pub struct Paint<'a> {
    pub origin: (f32, f32),
    pub limit: Option<f32>,
    pub colors: &'a [[f32; 4]],
    pub clip: [f32; 4],
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

impl Text {
    pub fn new() -> Self {
        Self {
            fonts: Fonts::new(false),
            fallback: Fallback::Off,
            wake: None,
            layouts_cx: LayoutContext::new(),
            scale: ScaleContext::new(),
            layouts: HashMap::new(),
            glyphs: HashMap::new(),
            shapes: HashMap::new(),
            shelf: Shelf { x: 0, y: 0, row: 0 },
            uploads: Vec::new(),
            frame: 0,
            scratch: Scratch::default(),
            pen: Vec::new(),
            atlas: 0,
            rasters: Vec::new(),
            released: Vec::new(),
            images: Vec::new(),
            hooks: Vec::new(),
            drawn: HashMap::new(),
        }
    }

    /// Asks for a canvas to be drawn off the main thread, unless its texture holds these pixels already.
    pub fn raster(&mut self, r: crate::canvas::Raster) {
        let stamp = r.stamp();
        if self.drawn.insert(r.id, stamp) != Some(stamp) {
            self.rasters.push(r);
        }
    }

    /// Asks for an image to be sent to the GPU off the main thread, unless it is there already.
    pub fn show(&mut self, image: &crate::image::Image) {
        if self.drawn.insert(image.texture(), 0).is_none() {
            self.images.push(image.clone());
        }
    }

    /// Asks for a canvas's paint hook to paint off the main thread, unless its texture holds these pixels already.
    pub fn hook(&mut self, h: crate::canvas::Hooked) {
        let stamp = h.stamp();
        if self.drawn.insert(h.id, stamp) != Some(stamp) {
            self.hooks.push(h);
        }
    }

    /// Lets go of a canvas's texture.
    pub fn release(&mut self, id: u32) {
        if self.drawn.remove(&id).is_some() {
            self.released.push(id);
        }
    }

    pub fn set_wake(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.wake = Some(std::sync::Arc::new(wake));
    }

    fn prepare(&mut self, spans: &[(&str, SpanStyle)]) {
        let han = Han::of(&spans.iter().map(|(t, _)| *t).collect::<String>());
        let mut complete = true;
        for (t, _) in spans {
            complete &= self.fonts.prepare(t, han);
        }
        if complete || !matches!(self.fallback, Fallback::Off) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = self.wake.clone();
        let seed = self.fonts.seed();
        let started = std::thread::Builder::new().name("font fallback".into()).spawn(move || {
            let _ = tx.send(Fonts::with_system(seed));
            if let Some(w) = wake {
                w();
            }
        });
        if started.is_ok() {
            self.fallback = Fallback::Loading(rx);
        }
    }

    pub fn wait_for_fallback(&mut self) {
        if let Fallback::Loading(rx) = &self.fallback
            && let Ok(fonts) = rx.recv()
        {
            self.use_fallback(fonts);
        }
    }

    fn use_fallback(&mut self, fonts: Fonts) {
        self.fonts = fonts;
        self.fallback = Fallback::Loaded;
        self.layouts.clear();
        self.clear();
    }

    /// Whether the system's fonts are still loading for text drawn since they were asked for.
    pub fn fallback_pending(&self) -> bool {
        matches!(self.fallback, Fallback::Loading(_))
    }

    pub fn begin_frame(&mut self) {
        self.frame += 1;
        if let Fallback::Loading(rx) = &self.fallback
            && let Ok(fonts) = rx.try_recv()
        {
            self.use_fallback(fonts);
        }
        if self.layouts.len() > 8000 {
            let frame = self.frame;
            self.layouts.retain(|_, (_, _, used)| frame - *used < 120);
        }
    }

    pub fn clear(&mut self) {
        self.layouts.clear();
        self.glyphs.clear();
        self.shapes.clear();
        self.shelf = Shelf { x: 0, y: 0, row: 0 };
        self.atlas += 1;
    }

    fn key_of(spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: Option<f32>) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (t, s) in spans {
            t.hash(&mut h);
            s.hash(&mut h);
        }
        size.to_bits().hash(&mut h);
        line.to_bits().hash(&mut h);
        wrap.map(|w| w.floor() as u32).hash(&mut h);
        h.finish()
    }

    fn layout(&mut self, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: Option<f32>) -> &Layout<u8> {
        let key = Self::key_of(spans, size, line, wrap);
        self.layout_at(key, spans, size, line, wrap)
    }

    fn layout_at(&mut self, key: u64, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: Option<f32>) -> &Layout<u8> {
        let frame = self.frame;
        if !self.layouts.contains_key(&key) {
            self.prepare(spans);
        }
        let fonts = &mut self.fonts.cx;
        let cx = &mut self.layouts_cx;
        let (mono, sans) = (self.fonts.mono.as_str(), self.fonts.sans.as_str());
        let entry = self.layouts.entry(key).or_insert_with(|| {
            profiling::scope!("shape");
            let text: String = spans.iter().map(|(t, _)| *t).collect();
            let mut b = cx.ranged_builder(fonts, &text, 1.0, true);
            b.push_default(StyleProperty::FontFamily(FontFamily::named(sans)));
            b.push_default(StyleProperty::FontSize(size));
            b.push_default(StyleProperty::LineHeight(LineHeight::Absolute(line)));
            if let Some(locale) = Han::of(&text).locale() {
                b.push_default(StyleProperty::Locale(Some(locale)));
            }
            let mut at = 0;
            for (t, s) in spans {
                let r = at..at + t.len();
                at = r.end;
                if r.is_empty() {
                    continue;
                }
                b.push(StyleProperty::FontWeight(FontWeight::new(s.weight as f32)), r.clone());
                if s.face == Face::Mono {
                    b.push(StyleProperty::FontFamily(FontFamily::named(mono)), r.clone());
                }
                if s.italic {
                    b.push(StyleProperty::FontStyle(FontStyle::Italic), r.clone());
                }
                if s.underline {
                    b.push(StyleProperty::Underline(true), r.clone());
                }
                b.push(StyleProperty::Brush(s.color), r);
            }
            let mut layout = b.build(&text);
            layout.break_all_lines(wrap);
            layout.align(Alignment::Start, AlignmentOptions::default());
            (layout, text, frame)
        });
        entry.2 = frame;
        &entry.0
    }

    pub fn source(&self, key: u64) -> Option<&str> {
        self.layouts.get(&key).map(|e| e.1.as_str())
    }

    pub fn rects_in(&self, key: u64, range: Range<usize>) -> Vec<[f32; 4]> {
        let Some((layout, _, _)) = self.layouts.get(&key) else { return Vec::new() };
        let a = parley::Cursor::from_byte_index(layout, range.start, parley::Affinity::Downstream);
        let b = parley::Cursor::from_byte_index(layout, range.end, parley::Affinity::Upstream);
        parley::Selection::new(a, b).geometry(layout).into_iter().map(|(bb, _)| [bb.x0 as f32, bb.y0 as f32, (bb.x1 - bb.x0) as f32, (bb.y1 - bb.y0) as f32]).collect()
    }

    pub fn pick(&self, key: u64, x: f32, y: f32, clicks: u32) -> Option<Range<usize>> {
        let (layout, _, _) = self.layouts.get(&key)?;
        let sel = match clicks {
            0 | 1 => parley::Selection::from_point(layout, x, y),
            2 => parley::Selection::word_from_point(layout, x, y),
            _ => parley::Selection::line_from_point(layout, x, y),
        };
        Some(sel.text_range())
    }

    pub fn measure(&mut self, text: &str, style: TextStyle, line: f32) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let span = SpanStyle { weight: style.weight as u16, ..SpanStyle::plain(style.face) };
        self.layout(&[(text, span)], style.size, line, None).full_width()
    }

    pub fn index_at(&mut self, text: &str, style: TextStyle, line: f32, x: f32) -> usize {
        let span = SpanStyle { weight: style.weight as u16, ..SpanStyle::plain(style.face) };
        parley::Cursor::from_point(self.layout(&[(text, span)], style.size, line, None), x, 0.0).index()
    }

    pub fn height(&mut self, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: f32) -> f32 {
        self.layout(spans, size, line, Some(wrap)).height()
    }

    pub fn byte_at(&mut self, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: f32, x: f32, y: f32) -> usize {
        let layout = self.layout(spans, size, line, Some(wrap));
        parley::Cursor::from_point(layout, x, y).index()
    }

    pub fn word_at(&mut self, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: f32, x: f32, y: f32) -> Range<usize> {
        let layout = self.layout(spans, size, line, Some(wrap));
        parley::Selection::word_from_point(layout, x, y).text_range()
    }

    pub fn caret(&mut self, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: f32, index: usize) -> [f32; 4] {
        let layout = self.layout(spans, size, line, Some(wrap));
        let b = parley::Cursor::from_byte_index(layout, index, parley::Affinity::Downstream).geometry(layout, 1.0);
        [b.x0 as f32, b.y0 as f32, (b.x1 - b.x0) as f32, (b.y1 - b.y0) as f32]
    }

    pub fn line_move(&mut self, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: f32, index: usize, to: Line) -> usize {
        let layout = self.layout(spans, size, line, Some(wrap));
        let at = parley::Selection::from_byte_index(layout, index, parley::Affinity::Downstream);
        let moved = match to {
            Line::Up => at.previous_line(layout, false),
            Line::Down => at.next_line(layout, false),
            Line::Start => at.line_start(layout, false),
            Line::End => at.line_end(layout, false),
        };
        moved.focus().index()
    }

    pub fn range_rects(&mut self, spans: &[(&str, SpanStyle)], size: f32, line: f32, wrap: f32, range: Range<usize>) -> Vec<[f32; 4]> {
        let layout = self.layout(spans, size, line, Some(wrap));
        let a = parley::Cursor::from_byte_index(layout, range.start, parley::Affinity::Downstream);
        let b = parley::Cursor::from_byte_index(layout, range.end, parley::Affinity::Upstream);
        parley::Selection::new(a, b).geometry(layout).into_iter().map(|(bb, _)| [bb.x0 as f32, bb.y0 as f32, (bb.x1 - bb.x0) as f32, (bb.y1 - bb.y0) as f32]).collect()
    }

    #[profiling::function]
    pub fn draw_spans(&mut self, out: &mut Vec<Quad>, block: Block, p: Paint) -> Drawn {
        let Block { spans, size, line, wrap } = block;
        let ellipsis = match p.limit {
            Some(_) => {
                let first = spans.first().map(|s| s.1).unwrap_or(SpanStyle::plain(Face::Prop));
                self.layout(&[("\u{2026}", first)], size, line, None).width()
            }
            None => 0.0,
        };
        let color = |i: u8| p.colors.get(i as usize).copied().unwrap_or(p.colors[0]);
        let mut s = std::mem::take(&mut self.scratch);
        s.clear();
        let mut underlines = Vec::new();
        let key = Self::key_of(spans, size, line, wrap);
        let layout = self.layout_at(key, spans, size, line, wrap);
        let (width, height) = (layout.width(), layout.height());
        let cut = p.limit.filter(|l| width > *l + 0.5).map(|l| l - ellipsis);
        let mut truncated = false;
        let mut end = 0.0f32;
        'lines: for l in layout.lines() {
            for item in l.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let font = run.run().font().clone();
                let fsize = run.run().font_size();
                let coords_from = s.coords.len();
                s.coords.extend_from_slice(run.run().normalized_coords());
                let coords_hash = s.coords[coords_from..].iter().fold(1469598103934665603u64, |h, c| (h ^ (*c as u16 as u64)).wrapping_mul(1099511628211));
                let style = run.style();
                let c = color(style.brush);
                if style.underline.is_some() {
                    let m = run.run().metrics();
                    let x = p.origin.0 + run.offset();
                    let y = (p.origin.1 + run.baseline() - m.underline_offset).round();
                    let w = run.advance();
                    let t = m.underline_size.max(1.0).round();
                    s.rules.push(([x, y, w, t], c));
                    underlines.push(([x, p.origin.1 + run.baseline() - m.ascent, w, m.ascent + m.descent], run.run().text_range()));
                }
                let glyphs_from = s.glyphs.len();
                for glyph in run.positioned_glyphs() {
                    if cut.is_some_and(|c| glyph.x + glyph.advance > c) {
                        truncated = true;
                        let glyphs_to = s.glyphs.len();
                        s.runs.push((font, coords_from..s.coords.len(), c, glyphs_from..glyphs_to));
                        break 'lines;
                    }
                    end = glyph.x + glyph.advance;
                    let gx = p.origin.0 + glyph.x;
                    let gy = p.origin.1 + glyph.y;
                    let sub = ((gx - gx.floor()) * 4.0).floor() as u8 & 3;
                    let key = GlyphKey { font: font.data.id(), index: font.index, glyph: glyph.id, size: fsize.to_bits(), coords: coords_hash, sub };
                    s.glyphs.push((key, gx.floor(), gy.round()));
                }
                let glyphs_to = s.glyphs.len();
                s.runs.push((font, coords_from..s.coords.len(), c, glyphs_from..glyphs_to));
            }
        }
        for (r, c) in &s.rules {
            out.push(Quad { pos: [r[0], r[1]], size: [r[2], r[3]], uv: [0.0; 4], color: *c, clip: p.clip, kind: 0, image: 0, pad: [0; 2] });
        }
        for (font, coords, c, glyphs) in &s.runs {
            for &(key, x, y) in &s.glyphs[glyphs.clone()] {
                let slot = match self.glyphs.get(&key) {
                    Some(slot) => *slot,
                    None => {
                        let slot = self.rasterize(key, font, &s.coords[coords.clone()]);
                        self.glyphs.insert(key, slot);
                        slot
                    }
                };
                if let Some(slot) = slot {
                    let a = ATLAS as f32;
                    out.push(Quad {
                        pos: [x + slot.left as f32, y - slot.top as f32],
                        size: [slot.w as f32, slot.h as f32],
                        uv: [slot.x as f32 / a, slot.y as f32 / a, slot.w as f32 / a, slot.h as f32 / a],
                        color: *c,
                        clip: p.clip,
                        kind: 1,
                        image: 0,
                        pad: [0; 2],
                    });
                }
            }
        }
        s.clear();
        self.scratch = s;
        if truncated {
            let first = spans.first().map(|s| s.1).unwrap_or(SpanStyle::plain(Face::Prop));
            self.draw_spans(out, Block { spans: &[("\u{2026}", first)], size, line, wrap: None }, Paint { origin: (p.origin.0 + end, p.origin.1), limit: None, colors: p.colors, clip: p.clip });
        }
        Drawn { key, width: if truncated { end + ellipsis } else { width }, height, underlines }
    }

    #[profiling::function]
    fn rasterize(&mut self, key: GlyphKey, font: &parley::FontData, coords: &[i16]) -> Option<Slot> {
        let font_ref = swash::FontRef::from_index(font.data.data(), font.index as usize)?;
        let mut scaler = self.scale.builder(font_ref).size(f32::from_bits(key.size)).hint(false).normalized_coords(coords.iter()).build();
        let image = Render::new(&[Source::Outline, Source::Bitmap(StrikeWith::BestFit)]).format(Format::Alpha).offset(Vector::new(key.sub as f32 / 4.0, 0.0)).render(&mut scaler, key.glyph as u16)?;
        self.store(image.placement, image.data)
    }

    fn store(&mut self, placement: swash::zeno::Placement, data: Vec<u8>) -> Option<Slot> {
        let (w, h) = (placement.width, placement.height);
        if w == 0 || h == 0 {
            return None;
        }
        let (x, y) = match self.shelf.alloc(w, h) {
            Some(at) => at,
            None => {
                self.glyphs.clear();
                self.shapes.clear();
                self.shelf = Shelf { x: 0, y: 0, row: 0 };
                self.atlas += 1;
                self.shelf.alloc(w, h)?
            }
        };
        self.uploads.push(Upload { x, y, w, h, data });
        Some(Slot { x, y, w, h, left: placement.left, top: placement.top })
    }

    pub fn shape(&mut self, out: &mut Vec<Quad>, key: u64, o: Outline, p: Paint) {
        let slot = match self.shapes.get(&key) {
            Some(slot) => *slot,
            None => {
                let (data, placement) = match o.path {
                    Data::Svg(d) => mask(d, &o),
                    Data::Commands(d) => mask(d, &o),
                };
                let slot = self.store(placement, data);
                self.shapes.insert(key, slot);
                slot
            }
        };
        if let Some(s) = slot {
            let a = ATLAS as f32;
            out.push(Quad {
                pos: [p.origin.0 + s.left as f32, p.origin.1 + s.top as f32],
                size: [s.w as f32, s.h as f32],
                uv: [s.x as f32 / a, s.y as f32 / a, s.w as f32 / a, s.h as f32 / a],
                color: p.colors.first().copied().unwrap_or([1.0; 4]),
                clip: p.clip,
                kind: 1,
                image: 0,
                pad: [0; 2],
            });
        }
    }
}
