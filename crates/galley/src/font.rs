use std::sync::Arc;

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::string::StringId;
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// A font a document is set in, TrueType or OpenType, read from its file. Sizes it reports are in ems, so a glyph's advance at 11 points is `advance * 11.0` points.
#[derive(Clone)]
pub struct Font {
    data: Arc<[u8]>,
    index: u32,
    pub(crate) name: String,
    pub(crate) units: f32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) cap_height: f32,
    pub(crate) italic_angle: f32,
    pub(crate) bounds: [f32; 4],
}

/// One step of a glyph's outline, in ems, with y going up from the baseline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font").field("name", &self.name).field("bytes", &self.data.len()).finish()
    }
}

impl Font {
    /// The font at `index` in a font file's bytes (0 unless the file is a collection), or `None` when they hold no font.
    pub fn new(data: impl Into<Arc<[u8]>>, index: u32) -> Option<Font> {
        let data = data.into();
        let font = FontRef::from_index(&data, index).ok()?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        let units = f32::from(metrics.units_per_em.max(1));
        let name = |id| font.localized_strings(id).english_or_first().map(|s| s.chars().collect::<String>());
        let name = name(StringId::POSTSCRIPT_NAME).or_else(|| name(StringId::FAMILY_NAME)).unwrap_or_default();
        let bounds = metrics.bounds.map_or([0.0, metrics.descent, units, metrics.ascent], |b| [b.x_min, b.y_min, b.x_max, b.y_max]).map(|v| v / units);
        Some(Font {
            name,
            units,
            ascent: metrics.ascent / units,
            descent: metrics.descent / units,
            cap_height: metrics.cap_height.unwrap_or(metrics.ascent * 0.7) / units,
            italic_angle: metrics.italic_angle,
            bounds,
            index,
            data,
        })
    }

    /// The font's PostScript name, or its family's when it has none.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How far above the baseline its tallest letters reach, in ems.
    pub fn ascent(&self) -> f32 {
        self.ascent
    }

    /// The glyph the font draws `c` with, or 0, the glyph fonts draw for what they lack.
    pub fn glyph(&self, c: char) -> u16 {
        self.font().and_then(|f| f.charmap().map(c)).map_or(0, |g| g.to_u32() as u16)
    }

    /// Whether the font has a glyph of its own for `c`.
    pub fn has(&self, c: char) -> bool {
        self.glyph(c) != 0
    }

    /// How far the pen moves after `glyph`, in ems.
    pub fn advance(&self, glyph: u16) -> f32 {
        self.font().and_then(|f| f.glyph_metrics(Size::unscaled(), LocationRef::default()).advance_width(GlyphId::new(u32::from(glyph)))).unwrap_or(0.0) / self.units
    }

    /// The outline of `glyph`, in ems.
    pub fn outline(&self, glyph: u16) -> Vec<Segment> {
        let mut pen = Pen { scale: 1.0 / self.units, out: Vec::new() };
        if let Some(outline) = self.font().and_then(|f| f.outline_glyphs().get(GlyphId::new(u32::from(glyph)))) {
            let _ = outline.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen);
        }
        pen.out
    }

    pub(crate) fn data(&self) -> &[u8] {
        &self.data
    }

    pub(crate) fn embeddable(&self) -> Option<Embed> {
        match self.data.get(..4)? {
            _ if self.index != 0 => None,
            b"ttcf" => None,
            b"OTTO" => Some(Embed::OpenType),
            _ => Some(Embed::TrueType),
        }
    }

    fn font(&self) -> Option<FontRef<'_>> {
        FontRef::from_index(&self.data, self.index).ok()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Embed {
    TrueType,
    OpenType,
}

struct Pen {
    scale: f32,
    out: Vec<Segment>,
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.out.push(Segment::Move(x * self.scale, y * self.scale));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.out.push(Segment::Line(x * self.scale, y * self.scale));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let s = self.scale;
        self.out.push(Segment::Quad(cx * s, cy * s, x * s, y * s));
    }

    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        let s = self.scale;
        self.out.push(Segment::Cubic(c0x * s, c0y * s, c1x * s, c1y * s, x * s, y * s));
    }

    fn close(&mut self) {
        self.out.push(Segment::Close);
    }
}
