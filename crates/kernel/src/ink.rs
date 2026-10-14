use std::sync::LazyLock;

pub type Color = [f32; 4];

/// The theme's semantic inks. Everything Graticule draws takes one of these, so a theme change restyles everything at once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub bg: Color,
    pub ink: Color,
    pub muted: Color,
    pub faint: Color,
    pub rule: Color,
    pub cursor: Color,
    pub hover: Color,
    pub selection: Color,
    pub field: Color,
    pub link: Color,
    pub visited: Color,
    pub danger: Color,
    pub warn: Color,
    pub ok: Color,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub fn of(appearance: Appearance) -> &'static Theme {
        match appearance {
            Appearance::Light => &LIGHT,
            Appearance::Dark => &DARK,
        }
    }

    pub fn texts(&self) -> [(&'static str, Color); 9] {
        [
            ("ink", self.ink),
            ("muted", self.muted),
            ("link", self.link),
            ("visited", self.visited),
            ("danger", self.danger),
            ("warn", self.warn),
            ("ok", self.ok),
            ("faint", self.faint),
            ("rule", self.rule),
        ]
    }
}

pub static LIGHT: LazyLock<Theme> = LazyLock::new(|| Theme {
    bg: oklch(0.98, 0.003, 100.0),
    ink: oklch(0.22, 0.005, 285.0),
    muted: oklch(0.5, 0.008, 285.0),
    faint: oklch(0.6, 0.008, 285.0),
    rule: oklch(0.78, 0.005, 285.0),
    cursor: oklch(0.925, 0.003, 100.0),
    hover: oklch(0.955, 0.003, 100.0),
    selection: oklch(0.92, 0.035, 262.0),
    field: oklch(0.95, 0.003, 100.0),
    link: oklch(0.48, 0.15, 262.0),
    visited: oklch(0.48, 0.15, 305.0),
    danger: oklch(0.48, 0.17, 28.0),
    warn: oklch(0.48, 0.11, 70.0),
    ok: oklch(0.48, 0.12, 150.0),
});

pub static DARK: LazyLock<Theme> = LazyLock::new(|| Theme {
    bg: oklch(0.2, 0.004, 285.0),
    ink: oklch(0.925, 0.004, 100.0),
    muted: oklch(0.7, 0.006, 285.0),
    faint: oklch(0.58, 0.008, 285.0),
    rule: oklch(0.41, 0.007, 285.0),
    cursor: oklch(0.285, 0.007, 285.0),
    hover: oklch(0.25, 0.006, 285.0),
    selection: oklch(0.32, 0.055, 262.0),
    field: oklch(0.25, 0.006, 285.0),
    link: oklch(0.77, 0.11, 260.0),
    visited: oklch(0.77, 0.12, 305.0),
    danger: oklch(0.77, 0.13, 26.0),
    warn: oklch(0.77, 0.12, 80.0),
    ok: oklch(0.77, 0.13, 151.0),
});

fn oklab_to_linear(l: f32, a: f32, b: f32) -> [f32; 3] {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_, -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_, -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_]
}

/// A color from OKLCH lightness, chroma and hue, reduced in chroma until it fits in sRGB.
pub fn oklch(l: f32, c: f32, h: f32) -> Color {
    let (sin, cos) = h.to_radians().sin_cos();
    let mut chroma = c;
    let mut rgb = oklab_to_linear(l, chroma * cos, chroma * sin);
    while chroma > 0.0 && rgb.iter().any(|v| !(-0.0001..=1.0001).contains(v)) {
        chroma = (chroma - 0.002).max(0.0);
        rgb = oklab_to_linear(l, chroma * cos, chroma * sin);
    }
    let encode = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    };
    [encode(rgb[0]), encode(rgb[1]), encode(rgb[2]), 1.0]
}

pub fn to_oklch(c: Color) -> (f32, f32, f32) {
    let f = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let (r, g, b) = (f(c[0]), f(c[1]), f(c[2]));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    (lightness, a.hypot(bb), bb.atan2(a).to_degrees().rem_euclid(360.0))
}

/// The WCAG contrast ratio between two colors.
pub fn contrast(a: Color, b: Color) -> f32 {
    let lum = |c: Color| {
        let f = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}
