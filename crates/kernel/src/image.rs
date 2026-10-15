//! Raster images: photographs, scans and screenshots, decoded from PNG or JPEG and shown in a box of cells.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};

/// A raster image: its pixels, decoded and shared by reference. Keep one in the facts as a `#[resource]`, so it is never copied into history, and decode it in a job, since decoding takes as long as the image is big.
#[derive(Clone, Debug, Default)]
pub struct Image(Option<Arc<Pixels>>);

#[derive(Debug)]
pub(crate) struct Pixels {
    id: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

static NEXT: AtomicU32 = AtomicU32::new(1);

/// What a file that isn't an image this can read says.
pub const NOT_AN_IMAGE: &str = "That isn't a PNG or JPEG image this can read";

impl Image {
    /// An image `width` by `height` pixels from its pixels, as RGBA with straight alpha, row by row.
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Image, &'static str> {
        if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
            return Err("The pixels don't fill the image");
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed) & 0x7fff_ffff;
        Ok(Image(Some(Arc::new(Pixels { id, width, height, rgba }))))
    }

    /// The image a PNG or JPEG file holds.
    pub fn decode(bytes: &[u8]) -> Result<Image, &'static str> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            return png_pixels(bytes).ok_or(NOT_AN_IMAGE).and_then(|(w, h, rgba)| Image::from_rgba(w, h, rgba));
        }
        if bytes.starts_with(&[0xff, 0xd8]) {
            return jpeg_pixels(bytes).ok_or(NOT_AN_IMAGE).and_then(|(w, h, rgba)| Image::from_rgba(w, h, rgba));
        }
        Err(NOT_AN_IMAGE)
    }

    /// Its width in pixels; 0 for no image.
    pub fn width(&self) -> u32 {
        self.0.as_ref().map_or(0, |p| p.width)
    }

    /// Its height in pixels; 0 for no image.
    pub fn height(&self) -> u32 {
        self.0.as_ref().map_or(0, |p| p.height)
    }

    /// Whether it is no image, as an image not loaded yet is.
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// The texture it is drawn from; 0 for no image.
    pub(crate) fn texture(&self) -> u32 {
        self.0.as_ref().map_or(0, |p| 0x8000_0000 | p.id)
    }

    pub(crate) fn pixels(&self) -> Option<&Arc<Pixels>> {
        self.0.as_ref()
    }

    pub(crate) fn watch(&self) -> Weak<Pixels> {
        self.0.as_ref().map_or_else(Weak::new, Arc::downgrade)
    }
}

impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        self.texture() == other.texture()
    }
}

impl Eq for Image {}

impl Hash for Image {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.texture().hash(state);
    }
}

fn png_pixels(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::GrayscaleAlpha => data.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Rgb => data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((info.width, info.height, rgba))
}

fn jpeg_pixels(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let options = zune_core::options::DecoderOptions::default().jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(zune_core::bytestream::ZCursor::new(bytes), options);
    let rgba = decoder.decode().ok()?;
    let (w, h) = decoder.dimensions()?;
    Some((w as u32, h as u32, rgba))
}

/// The image's levels for the GPU, each half the one before, the first no larger than `most` on either side: a box filter weighted by alpha, so transparent pixels don't darken their neighbours.
pub(crate) fn levels(p: &Pixels, most: u32) -> Vec<(u32, u32, Vec<u8>)> {
    let mut levels: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    let (mut w, mut h) = (p.width, p.height);
    let mut now: Option<Vec<u8>> = None;
    loop {
        let fits = w <= most && h <= most;
        if fits {
            levels.push((w, h, now.clone().unwrap_or_else(|| p.rgba.clone())));
        }
        if w == 1 && h == 1 {
            return levels;
        }
        let from = now.as_deref().unwrap_or(&p.rgba);
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; nw as usize * nh as usize * 4];
        for y in 0..nh {
            for x in 0..nw {
                let mut sum = [0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                    let px = &from[(sy * w + sx) as usize * 4..][..4];
                    let a = f32::from(px[3]);
                    for k in 0..3 {
                        sum[k] += f32::from(px[k]) * a;
                    }
                    sum[3] += a;
                }
                let out = &mut next[(y * nw + x) as usize * 4..][..4];
                for k in 0..3 {
                    out[k] = if sum[3] > 0.0 { (sum[k] / sum[3]).round() as u8 } else { 0 };
                }
                out[3] = (sum[3] / 4.0).round() as u8;
            }
        }
        (w, h, now) = (nw, nh, Some(next));
    }
}

impl ledger::Field for Image {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), _: ledger::ErasedPath, _: &mut Vec<ledger::Op>) {}

    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn value(&self) -> ledger::Value {
        ledger::Value::Unit
    }

    fn from_value(_: &ledger::Value) -> Option<Self> {
        None
    }
}
