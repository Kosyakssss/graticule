use crate::grid::Area;
use crate::px::Rect;
use crate::text::{Face, Text, TextStyle};

pub const FONT_POINTS: f32 = 13.0;
pub const LINE: f32 = 1.6;

/// The size of a cell, in whole points and in device pixels, and the window's size in cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub scale: f32,
    pub size: f32,
    pub cell_w: f32,
    pub cell_h: f32,
    pub width: f32,
    pub height: f32,
}

impl Metrics {
    pub fn measure(text: &mut Text, scale: f32, width: u32, height: u32) -> Self {
        let probe = TextStyle { face: Face::Mono, size: FONT_POINTS, weight: 400.0 };
        let advance = text.measure("0000000000", probe, FONT_POINTS * LINE) / 10.0;
        let cell_w_pt = advance.round().max(1.0);
        let font_pt = FONT_POINTS * cell_w_pt / advance;
        let cell_h_pt = (font_pt * LINE).round();
        Self { scale, size: font_pt * scale, cell_w: (cell_w_pt * scale).round(), cell_h: (cell_h_pt * scale).round(), width: width as f32, height: height as f32 }
    }

    pub fn cols(&self) -> i32 {
        (self.width / self.cell_w).floor() as i32
    }

    pub fn rows(&self) -> i32 {
        (self.height / self.cell_h).floor() as i32
    }

    pub fn screen(&self) -> Area {
        Area::new(0, 0, self.cols(), self.rows())
    }

    pub fn rect(&self, a: Area) -> Rect {
        Rect::new(a.col as f32 * self.cell_w, a.row as f32 * self.cell_h, a.cols.max(0) as f32 * self.cell_w, a.rows.max(0) as f32 * self.cell_h)
    }

    pub fn hair(&self) -> f32 {
        self.scale.round().max(1.0)
    }

    pub fn mono(&self) -> TextStyle {
        TextStyle { face: Face::Mono, size: self.size, weight: 400.0 }
    }

    pub fn sans(&self, weight: f32) -> TextStyle {
        TextStyle { face: Face::Prop, size: self.size, weight }
    }

    pub fn cell_at(&self, x: f32, y: f32) -> (i32, i32) {
        ((x / self.cell_w).floor() as i32, (y / self.cell_h).floor() as i32)
    }
}
