//! Colors as facts, in OKLCH, and reading them from what people type: `#3366cc` or `oklch(0.63 0.26 290)`.

use std::fmt;

use graticule_macros::Facts;

use crate::ink::{Color, oklch, to_oklch};

/// A color as OKLCH, in whole numbers so a slider can set each part: lightness in thousandths (0 to 1000), chroma in thousandths (0 to 400) and hue in tenths of a degree (0 to 3599). A chroma past what the screen can show is drawn reduced until it fits.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Oklch {
    pub l: u16,
    pub c: u16,
    pub h: u16,
}

/// One part of an `Oklch`, as a color slider sets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Lightness,
    Chroma,
    Hue,
}

impl Channel {
    /// Its name, as a slider for it is called.
    pub fn name(self) -> &'static str {
        match self {
            Channel::Lightness => "Lightness",
            Channel::Chroma => "Chroma",
            Channel::Hue => "Hue",
        }
    }

    /// This part of `color`, as the whole number a slider sets.
    pub fn of(self, color: Oklch) -> u16 {
        match self {
            Channel::Lightness => color.l,
            Channel::Chroma => color.c,
            Channel::Hue => color.h,
        }
    }

    /// A value of this part, from its whole number, as people read it: `0.630` for lightness and chroma, `290.0°` for hue.
    pub fn show(self, whole: i128) -> String {
        match self {
            Channel::Hue => format!("{:.1}°", whole as f32 / 10.0),
            _ => format!("{:.3}", whole as f32 / 1000.0),
        }
    }

    /// The color `base` with this part at `share` of its range, from 0 to 1.
    pub fn at(self, base: Oklch, share: f32) -> Oklch {
        let share = share.clamp(0.0, 1.0);
        match self {
            Channel::Lightness => Oklch { l: (share * 1000.0).round() as u16, ..base },
            Channel::Chroma => Oklch { c: (share * f32::from(CHROMA)).round() as u16, ..base },
            Channel::Hue => Oklch { h: ((share * f32::from(HUE - 1)).round() as u16).min(HUE - 1), ..base },
        }
    }
}

/// The most chroma an `Oklch` holds, in thousandths.
pub const CHROMA: u16 = 400;
/// The hue past the last, in tenths of a degree.
pub const HUE: u16 = 3600;

/// What doesn't read as a color says this.
pub const NOT_A_COLOR: &str = "That isn't a color: try #3366cc or oklch(0.63 0.26 290)";

impl Oklch {
    /// The color with lightness `l` from 0 to 1, chroma `c` from 0 to 0.4 and hue `h` in degrees, rounded to what an `Oklch` holds.
    pub fn new(l: f32, c: f32, h: f32) -> Oklch {
        Oklch { l: (l.clamp(0.0, 1.0) * 1000.0).round() as u16, c: (c.clamp(0.0, 0.4) * 1000.0).round() as u16, h: ((h.rem_euclid(360.0) * 10.0).round() as u16) % HUE }
    }

    /// The color to draw, in sRGB.
    pub fn color(self) -> Color {
        oklch(f32::from(self.l) / 1000.0, f32::from(self.c) / 1000.0, f32::from(self.h) / 10.0)
    }

    /// The color as the hex of its sRGB, `#3366cc`.
    pub fn hex(self) -> String {
        let [r, g, b, _] = self.color().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

impl fmt::Display for Oklch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "oklch({:.3} {:.3} {:.1})", f32::from(self.l) / 1000.0, f32::from(self.c) / 1000.0, f32::from(self.h) / 10.0)
    }
}

/// The color `text` names: hex as `#3366cc`, `3366cc` or `#36c`, or OKLCH as `oklch(0.63 0.26 290)`, with the lightness also as `63%` and the hue also as `290deg`, or the three numbers alone.
pub fn read_color(text: &str) -> Result<Oklch, &'static str> {
    let text = text.trim().to_lowercase();
    let digits = text.strip_prefix('#').unwrap_or(&text);
    if matches!(digits.len(), 3 | 6) && digits.chars().all(|c| c.is_ascii_hexdigit()) {
        let wide: String = if digits.len() == 3 { digits.chars().flat_map(|c| [c, c]).collect() } else { digits.to_string() };
        let channel = |k: usize| u8::from_str_radix(&wide[k..k + 2], 16).map(|v| f32::from(v) / 255.0);
        let (Ok(r), Ok(g), Ok(b)) = (channel(0), channel(2), channel(4)) else { return Err(NOT_A_COLOR) };
        let (l, c, h) = to_oklch([r, g, b, 1.0]);
        return Ok(Oklch::new(l, if c < 0.0005 { 0.0 } else { c }, if c < 0.0005 { 0.0 } else { h }));
    }
    let inside = text.strip_prefix("oklch(").and_then(|t| t.strip_suffix(')')).unwrap_or(&text);
    let parts: Vec<&str> = inside.split(|c: char| c.is_whitespace() || c == ',' || c == '/').filter(|p| !p.is_empty()).collect();
    let [l, c, h] = parts.as_slice() else { return Err(NOT_A_COLOR) };
    let l = match l.strip_suffix('%') {
        Some(percent) => percent.parse::<f32>().map(|p| p / 100.0),
        None => l.parse::<f32>(),
    };
    let (Ok(l), Ok(c), Ok(h)) = (l, c.parse::<f32>(), h.trim_end_matches("deg").parse::<f32>()) else { return Err(NOT_A_COLOR) };
    if !(0.0..=1.0).contains(&l) || !(0.0..=0.4).contains(&c) || !h.is_finite() {
        return Err(NOT_A_COLOR);
    }
    Ok(Oklch::new(l, c, h))
}
