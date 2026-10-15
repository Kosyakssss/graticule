use std::path::Path;
use std::time::Instant;

use crate::gpu::{Gpu, Quad};
use crate::look::Look;
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
    /// The quads of the frame rendered last.
    pub fn quads(&self) -> &[Quad] {
        &self.quads
    }
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

    pub fn render(&mut self, look: Look, paint: impl Fn(&mut Painter)) -> Picture {
        let mut paint_ms = 0.0;
        for _ in 0..2 {
            let started = Instant::now();
            self.text.wait_for_fallback();
            self.text.begin_frame();
            self.quads.clear();
            let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, look);
            paint(&mut painter);
            paint_ms = started.elapsed().as_secs_f32() * 1000.0;
            if !self.text.fallback_pending() {
                break;
            }
        }
        for u in self.text.uploads.drain(..) {
            self.gpu.upload(u.x, u.y, u.w, u.h, &u.data);
        }
        for id in self.text.released.drain(..) {
            self.gpu.remove_image(id);
        }
        for r in self.text.rasters.drain(..) {
            self.gpu.raster(r);
        }
        for image in self.text.images.drain(..) {
            self.gpu.picture(image);
        }
        for h in self.text.hooks.drain(..) {
            self.gpu.hook(h);
        }
        let rgba = self.gpu.snapshot(&self.quads, look.theme.bg);
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
    pub fn render_face<F: Face>(&mut self, look: Look, face: &mut F) -> Picture {
        let look = face.look(look);
        let appearance = look.appearance;
        let mut paint_ms = 0.0;
        for _ in 0..2 {
            let started = Instant::now();
            self.text.wait_for_fallback();
            self.text.begin_frame();
            self.quads.clear();
            let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, look);
            face.paint(&mut painter, Seen { appearance, ..Seen::default() });
            paint_ms = started.elapsed().as_secs_f32() * 1000.0;
            if !self.text.fallback_pending() {
                break;
            }
        }
        if let Some(kept) = face.kept() {
            self.quads.clear();
            kept.all(&mut self.quads);
        }
        for u in self.text.uploads.drain(..) {
            self.gpu.upload(u.x, u.y, u.w, u.h, &u.data);
        }
        for id in self.text.released.drain(..) {
            self.gpu.remove_image(id);
        }
        for r in self.text.rasters.drain(..) {
            self.gpu.raster(r);
        }
        for image in self.text.images.drain(..) {
            self.gpu.picture(image);
        }
        for h in self.text.hooks.drain(..) {
            self.gpu.hook(h);
        }
        let rgba = self.gpu.snapshot(&self.quads, look.theme.bg);
        Picture { width: self.width, height: self.height, rgba, paint_ms }
    }
}
