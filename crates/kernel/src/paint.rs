use crate::chars::{self, Pen, Shape};
use crate::gpu::Quad;
use crate::grid::Area;
use crate::ink::{Color, Theme};
use crate::look::{Look, Style};
use crate::metrics::Metrics;
use crate::px::Rect;
use crate::text::{Block, Data, Drawn, Face, Outline, Paint, SpanStyle, Text, TextStyle};

pub struct Painter<'a> {
    pub quads: &'a mut Vec<Quad>,
    pub text: &'a mut Text,
    pub m: Metrics,
    pub theme: Theme,
    pub style: Style,
    clip: [f32; 4],
}

impl<'a> Painter<'a> {
    pub fn new(quads: &'a mut Vec<Quad>, text: &'a mut Text, m: Metrics, look: Look) -> Self {
        Self { quads, text, m, theme: look.theme, style: look.style, clip: [0.0, 0.0, m.width, m.height] }
    }

    pub fn clip(&self) -> [f32; 4] {
        self.clip
    }

    pub fn set_clip(&mut self, clip: [f32; 4]) -> [f32; 4] {
        std::mem::replace(&mut self.clip, clip)
    }

    pub fn clip_to(&mut self, r: Rect) -> [f32; 4] {
        let c = self.clip;
        self.set_clip([c[0].max(r.x), c[1].max(r.y), c[2].min(r.right()), c[3].min(r.bottom())])
    }

    pub fn rect(&self, a: Area) -> Rect {
        self.m.rect(a)
    }

    pub fn fill(&mut self, r: Rect, color: Color) {
        self.quads.push(Quad { pos: [r.x, r.y], size: [r.w, r.h], uv: [0.0; 4], color, clip: self.clip, kind: 0, image: 0, pad: [0; 2] });
    }

    pub fn fill_area(&mut self, a: Area, color: Color) {
        let r = self.rect(a);
        self.fill(r, color);
    }

    pub fn grid(&mut self) {
        let m = self.m;
        let t = m.hair();
        let color = self.theme.rule;
        for row in 1..=m.rows() {
            for col in 1..=m.cols() {
                let (x, y) = (col as f32 * m.cell_w, row as f32 * m.cell_h);
                self.fill(Rect::new(x - t, y - t, t, t), color);
            }
        }
    }

    pub fn cell(&mut self, c: char, r: Rect, color: Color) -> bool {
        let mut pen = Pen { t: self.m.hair(), rects: std::mem::take(&mut self.text.pen), alpha: 1.0 };
        pen.rects.clear();
        let drawn = match chars::draw(c, r, &mut pen) {
            None => false,
            Some(Shape::Rects) => {
                let mut color = color;
                color[3] *= pen.alpha;
                for q in &pen.rects {
                    self.fill(*q, color);
                }
                true
            }
            Some(Shape::Path(path, stroke)) => {
                let key = {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    (c, r.w.to_bits(), r.h.to_bits(), pen.t.to_bits()).hash(&mut h);
                    h.finish()
                };
                let clip = self.clip;
                self.text.shape(self.quads, key, Outline { path: Data::Svg(&path), stroke, even_odd: false, scale: 1.0 }, Paint { origin: (r.x, r.y), limit: None, colors: &[color], clip });
                true
            }
        };
        self.text.pen = pen.rects;
        drawn
    }

    pub fn cells(&mut self, at: (f32, f32), cols: i32, s: &str, style: TextStyle, color: Color) -> f32 {
        let (cw, ch) = (self.m.cell_w, self.m.cell_h);
        let mut run: Option<(usize, i32)> = None;
        let mut col = 0;
        let mut end = s.len();
        for (i, c) in s.char_indices() {
            if col >= cols {
                end = i;
                break;
            }
            if chars::drawn(c) || !c.is_ascii() {
                if let Some((from, start)) = run.take() {
                    self.plain(&s[from..i], style, (at.0 + start as f32 * cw, at.1), None, color);
                }
                let x = at.0 + col as f32 * cw;
                if chars::drawn(c) {
                    self.cell(c, Rect::new(x, at.1, cw, ch), color);
                } else {
                    let one = &s[i..i + c.len_utf8()];
                    let w = self.measure(one, style);
                    self.plain(one, style, ((x + (cw * crate::text::cells(c) as f32 - w) / 2.0).round(), at.1), None, color);
                    col += crate::text::cells(c) as i32 - 1;
                }
            } else if run.is_none() {
                run = Some((i, col));
            }
            col += 1;
        }
        if let Some((from, start)) = run {
            self.plain(&s[from..end], style, (at.0 + start as f32 * cw, at.1), None, color);
        }
        col as f32 * cw
    }

    pub fn text(&mut self, s: &str, style: TextStyle, at: (f32, f32), limit: Option<f32>, color: Color) -> Drawn {
        if style.face == Face::Mono {
            let cols = limit.map_or(i32::MAX, |l| (l / self.m.cell_w).floor() as i32);
            if s.chars().any(|c| chars::drawn(c) || !c.is_ascii()) && !s.chars().any(crate::text::shaped) {
                let width = self.cells(at, cols, s, style, color);
                return Drawn { key: 0, width, height: self.m.cell_h, underlines: Vec::new() };
            }
        }
        self.plain(s, style, at, limit, color)
    }

    pub fn write(&mut self, a: Area, s: &str, style: TextStyle, color: Color) -> f32 {
        let r = self.rect(a);
        self.text(s, style, (r.x, r.y), Some(r.w), color).width
    }

    fn plain(&mut self, s: &str, style: TextStyle, at: (f32, f32), limit: Option<f32>, color: Color) -> Drawn {
        let span = SpanStyle { weight: style.weight as u16, ..SpanStyle::plain(style.face) };
        let line = self.m.cell_h;
        let clip = self.clip;
        self.text.draw_spans(self.quads, Block { spans: &[(s, span)], size: style.size, line, wrap: None }, Paint { origin: at, limit, colors: &[color], clip })
    }

    pub fn measure(&mut self, s: &str, style: TextStyle) -> f32 {
        let line = self.m.cell_h;
        self.text.measure(s, style, line)
    }
}
