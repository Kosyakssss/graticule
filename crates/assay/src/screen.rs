use std::time::Instant;

use kernel::gpu::Quad;
use kernel::ink::Appearance;
use kernel::metrics::Metrics;
use kernel::paint::Painter;
use kernel::shell::{Face, Seen};
use kernel::text::Text;

pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1280;
pub const SCALE: f32 = 2.0;

pub struct Screen {
    pub text: Text,
    pub metrics: Metrics,
    quads: Vec<Quad>,
}

impl Screen {
    pub fn new() -> Screen {
        let mut text = Text::new();
        let metrics = Metrics::measure(&mut text, SCALE, WIDTH, HEIGHT);
        Screen { text, metrics, quads: Vec::new() }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.metrics = Metrics::measure(&mut self.text, SCALE, width, height);
    }

    pub fn cells(&mut self, cols: i32, rows: i32) {
        let m = self.metrics;
        self.resize((cols as f32 * m.cell_w) as u32, (rows as f32 * m.cell_h) as u32);
    }

    pub fn paint<F: Face>(&mut self, face: &mut F) -> u32 {
        let started = Instant::now();
        self.text.begin_frame();
        self.quads.clear();
        let look = face.look(kernel::look::Look::of(Appearance::Light));
        let mut painter = Painter::new(&mut self.quads, &mut self.text, self.metrics, look);
        face.paint(&mut painter, Seen::default());
        started.elapsed().as_micros().min(u32::MAX as u128) as u32
    }

    pub fn settle<F: Face>(&mut self, face: &mut F) {
        self.paint(face);
        for _ in 0..3 {
            if !self.text.fallback_pending() {
                break;
            }
            self.text.wait_for_fallback();
            self.paint(face);
        }
    }
}
