use std::f64::consts::TAU;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use graticule::kernel::wgpu;

/// A photograph or a scan at its full depth: sixteen bits a channel, gray or red, green and blue, row by row, and how many pixels it has to the inch when the file says.
pub struct Scan {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub color: bool,
    pub pixels: Vec<u16>,
    pub dpi: Option<f64>,
    /// How many bits a channel the file held.
    pub depth: u8,
    pub(crate) gpu: Mutex<Option<(u64, wgpu::Texture)>>,
}

/// The widest and tallest a scan is shown at, in pixels, as the GPU takes it; a larger one is halved until it fits.
const MOST: u32 = 8_192;

static NEXT: AtomicU64 = AtomicU64::new(1);

impl Scan {
    fn new(width: u32, height: u32, color: bool, pixels: Vec<u16>, dpi: Option<f64>, depth: u8) -> Scan {
        let mut scan = Scan { id: NEXT.fetch_add(1, Ordering::Relaxed), width, height, color, pixels, dpi, depth, gpu: Mutex::new(None) };
        while scan.width > MOST || scan.height > MOST {
            scan = scan.halved();
        }
        scan
    }

    fn channels(&self) -> usize {
        if self.color { 3 } else { 1 }
    }

    fn halved(self) -> Scan {
        let (w, h, c) = ((self.width / 2).max(1), (self.height / 2).max(1), self.channels());
        let mut out = vec![0u16; w as usize * h as usize * c];
        for y in 0..h as usize {
            for x in 0..w as usize {
                for k in 0..c {
                    let at = |dx: usize, dy: usize| u32::from(self.pixels[((y * 2 + dy).min(self.height as usize - 1) * self.width as usize + (x * 2 + dx).min(self.width as usize - 1)) * c + k]);
                    out[(y * w as usize + x) * c + k] = ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4) as u16;
                }
            }
        }
        Scan { width: w, height: h, pixels: out, dpi: self.dpi.map(|d| d / 2.0), ..self }
    }

    /// Its pixels as the GPU takes them: red, green, blue and alpha, sixteen bits each.
    pub(crate) fn rgba(&self) -> Vec<u8> {
        let c = self.channels();
        self.pixels.chunks_exact(c).flat_map(|p| if c == 3 { [p[0], p[1], p[2], u16::MAX] } else { [p[0], p[0], p[0], u16::MAX] }).flat_map(u16::to_le_bytes).collect()
    }

    /// The scan a PNG or JPEG file holds, at the depth it was saved at.
    pub fn read(bytes: &[u8]) -> Result<Scan, String> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            return png(bytes).ok_or_else(|| "The PNG can't be read".to_string());
        }
        if bytes.starts_with(&[0xff, 0xd8]) {
            return jpeg(bytes).ok_or_else(|| "The JPEG can't be read".to_string());
        }
        Err("That isn't a PNG or JPEG image".to_string())
    }

    /// A made-up film scan of a test target at 2400 pixels to the inch: a step wedge, bars of every width, a star of spokes, film grain, and rings too faint for eight bits that only pulling the levels in shows.
    pub fn sample() -> Scan {
        let (w, h) = (2_400u32, 1_600u32);
        let mut grain = 0x2545_f491_4f6c_dd1du64;
        let mut noise = move || {
            grain ^= grain << 13;
            grain ^= grain >> 7;
            grain ^= grain << 17;
            (grain >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let mut pixels = Vec::with_capacity(w as usize * h as usize);
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (f64::from(x), f64::from(y));
                let mut v = 0.12 + 0.04 * fx / f64::from(w);
                if (80.0..280.0).contains(&fy) && (100.0..2_300.0).contains(&fx) {
                    let step = ((fx - 100.0) / (2_200.0 / 21.0)).floor();
                    v = 10f64.powf(-step * 0.15) * 0.95;
                }
                if (420.0..1_420.0).contains(&fy) && (100.0..1_000.0).contains(&fx) {
                    let group = ((fy - 420.0) / 125.0).floor() as i32;
                    let width = 2f64.powi(6 - group).max(1.0);
                    let across = fx - 100.0;
                    let bar = (across / width).floor() as i64;
                    v = if across < width * 6.0 && bar % 2 == 0 && ((fy - 420.0) % 125.0) < 100.0 { 0.85 } else { 0.12 };
                }
                let (dx, dy) = (fx - 1_500.0, fy - 900.0);
                let r = (dx * dx + dy * dy).sqrt();
                if r < 420.0 {
                    let a = dy.atan2(dx);
                    v = if (a / TAU * 72.0).rem_euclid(2.0) < 1.0 { 0.8 } else { 0.15 };
                }
                let (ex, ey) = (fx - 2_050.0, fy - 1_150.0);
                let s = (ex * ex + ey * ey).sqrt();
                if s < 300.0 {
                    v = 0.5 + 0.003 * (s / 18.0).sin();
                }
                v += noise() * 0.006;
                pixels.push((v.clamp(0.0, 1.0) * 65_535.0).round() as u16);
            }
        }
        Scan::new(w, h, false, pixels, Some(2_400.0), 16)
    }
}

fn png(bytes: &[u8]) -> Option<Scan> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let dpi = reader.info().pixel_dims.filter(|d| d.unit == png::Unit::Meter && d.xppu > 0).map(|d| f64::from(d.xppu) * 0.0254);
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];
    let wide = info.bit_depth == png::BitDepth::Sixteen;
    let samples: Vec<u16> = if wide { data.chunks_exact(2).map(|b| u16::from_be_bytes([b[0], b[1]])).collect() } else { data.iter().map(|&b| u16::from(b) * 257).collect() };
    let (color, take): (bool, &[usize]) = match info.color_type {
        png::ColorType::Grayscale => (false, &[0]),
        png::ColorType::GrayscaleAlpha => (false, &[0, 1]),
        png::ColorType::Rgb => (true, &[0, 1, 2]),
        png::ColorType::Rgba => (true, &[0, 1, 2, 3]),
        png::ColorType::Indexed => return None,
    };
    let stride = take.len();
    let keep = if color { 3 } else { 1 };
    let pixels = samples.chunks_exact(stride).flat_map(|p| p[..keep].to_vec()).collect();
    Some(Scan::new(info.width, info.height, color, pixels, dpi, if wide { 16 } else { 8 }))
}

fn jpeg(bytes: &[u8]) -> Option<Scan> {
    let options = zune_core::options::DecoderOptions::default().jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::RGB);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(zune_core::bytestream::ZCursor::new(bytes), options);
    let rgb = decoder.decode().ok()?;
    let (w, h) = decoder.dimensions()?;
    Some(Scan::new(w as u32, h as u32, true, rgb.into_iter().map(|b| u16::from(b) * 257).collect(), None, 8))
}

/// How many of a scan's pixels fall at each of 256 levels, darkest first.
pub struct Histogram(pub Vec<u32>);

impl Histogram {
    pub fn of(scan: &Scan) -> Histogram {
        let mut bins = vec![0u32; 256];
        let c = scan.channels();
        for p in scan.pixels.chunks_exact(c) {
            let v = p.iter().map(|v| u32::from(*v)).sum::<u32>() / c as u32;
            bins[(v >> 8) as usize] += 1;
        }
        Histogram(bins)
    }

    /// The levels below which and above which a share `cut` of the pixels lie, from 0 to 65535: where stretching the levels puts black and white.
    pub fn ends(&self, cut: f64) -> (u16, u16) {
        let total: u64 = self.0.iter().map(|n| u64::from(*n)).sum();
        let edge = (total as f64 * cut) as u64;
        let find = |order: Vec<usize>| {
            let mut seen = 0;
            order
                .into_iter()
                .find(|k| {
                    seen += u64::from(self.0[*k]);
                    seen > edge
                })
                .unwrap_or(0)
        };
        let low = find((0..self.0.len()).collect());
        let high = find((0..self.0.len()).rev().collect());
        ((low * 256) as u16, ((high * 256) + 255) as u16)
    }
}
