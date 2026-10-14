use std::path::Path;
use std::time::Instant;

use crate::gpu::{Gpu, Quad};
use crate::ink::{Appearance, Theme};
use crate::metrics::Metrics;
use crate::paint::Painter;
use crate::shell::{Face, Seen};
use crate::text::Text;

/// Renders frames without a window, for Specimen's pages, proofs and laws that compare pixels.
pub struct Offscreen {
    gpu: Gpu,
    pub text: Text,
    pub metrics: Metrics,
    quads: Vec<Quad>,
    width: u32,
    height: u32,
}

pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub paint_ms: f32,
}

impl Offscreen {
    pub fn new(width: u32, height: u32, scale: f32) -> Self {
        let mut text = Text::new();
        let metrics = Metrics::measure(&mut text, scale, width, height);
        Self { gpu: Gpu::new(None, (width, height)), text, metrics, quads: Vec::new(), width, height }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) == (self.width, self.height) {
            return;
        }
        self.gpu.resize(width, height);
        self.metrics = Metrics::measure(&mut self.text, self.metrics.scale, width, height);
        (self.width, self.height) = (width, height);
    }

    pub fn render(&mut self, theme: Theme, paint: impl FnOnce(&mut Painter)) -> Picture {
        let started = Instant::now();
        self.text.wait_for_fallback();
        self.text.begin_frame();
        self.quads.clear();
        let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, theme);
        paint(&mut painter);
        let paint_ms = started.elapsed().as_secs_f32() * 1000.0;
        for u in self.text.uploads.drain(..) {
            self.gpu.upload(u.x, u.y, u.w, u.h, &u.data);
        }
        let rgba = self.gpu.snapshot(&self.quads, theme.bg);
        Picture { width: self.width, height: self.height, rgba, paint_ms }
    }
}

impl Picture {
    pub fn save_png(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().and_then(|mut w| w.write_image_data(&self.rgba)).map_err(|e| e.to_string())
    }
}

impl Offscreen {
    /// Renders one frame of a face, from its kept quads when it keeps them.
    pub fn render_face<F: Face>(&mut self, appearance: Appearance, face: &mut F) -> Picture {
        let theme = face.theme(appearance);
        let started = Instant::now();
        self.text.wait_for_fallback();
        self.text.begin_frame();
        self.quads.clear();
        let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, theme);
        face.paint(&mut painter, Seen { appearance, ..Seen::default() });
        let paint_ms = started.elapsed().as_secs_f32() * 1000.0;
        if let Some(kept) = face.kept() {
            self.quads.clear();
            kept.all(&mut self.quads);
        }
        for u in self.text.uploads.drain(..) {
            self.gpu.upload(u.x, u.y, u.w, u.h, &u.data);
        }
        let rgba = self.gpu.snapshot(&self.quads, theme.bg);
        Picture { width: self.width, height: self.height, rgba, paint_ms }
    }
}
