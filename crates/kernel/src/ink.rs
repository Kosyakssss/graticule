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

    /// The color of a named ink.
    pub fn get(&self, ink: Ink) -> Color {
        match ink {
            Ink::Background => self.bg,
            Ink::Ink => self.ink,
            Ink::Muted => self.muted,
            Ink::Faint => self.faint,
            Ink::Rule => self.rule,
            Ink::Cursor => self.cursor,
            Ink::Hover => self.hover,
            Ink::Selection => self.selection,
            Ink::Field => self.field,
            Ink::Link => self.link,
            Ink::Visited => self.visited,
            Ink::Danger => self.danger,
            Ink::Warn => self.warn,
            Ink::Ok => self.ok,
        }
    }
}

/// One of the theme's semantic inks, by name, for code that draws in a chosen ink rather than a fixed color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Ink {
    Background,
    #[default]
    Ink,
    Muted,
    Faint,
    Rule,
    Cursor,
    Hover,
    Selection,
    Field,
    Link,
    Visited,
    Danger,
    Warn,
    Ok,
}

impl Ink {
    /// Every ink, surfaces first, then the inks for text and lines.
    pub const ALL: [Ink; 14] =
        [Ink::Background, Ink::Field, Ink::Hover, Ink::Cursor, Ink::Selection, Ink::Ink, Ink::Muted, Ink::Faint, Ink::Rule, Ink::Link, Ink::Visited, Ink::Danger, Ink::Warn, Ink::Ok];

    /// The ink's name, as theme files and Specimen call it.
    pub fn name(self) -> &'static str {
        match self {
            Ink::Background => "background",
            Ink::Ink => "ink",
            Ink::Muted => "muted",
            Ink::Faint => "faint",
            Ink::Rule => "rule",
            Ink::Cursor => "cursor",
            Ink::Hover => "hover",
            Ink::Selection => "selection",
            Ink::Field => "field",
            Ink::Link => "link",
            Ink::Visited => "visited",
            Ink::Danger => "danger",
            Ink::Warn => "warn",
            Ink::Ok => "ok",
        }
    }

    /// Whether the ink is a surface that things are drawn on, rather than one that text and lines are drawn in.
    pub fn surface(self) -> bool {
        matches!(self, Ink::Background | Ink::Field | Ink::Hover | Ink::Cursor | Ink::Selection)
    }

    /// The contrast an ink for text must reach on the background, by the theme's law: 4.5, or 3 for faint text. Surfaces and rules have none.
    pub fn needs(self) -> Option<f32> {
        match self {
            Ink::Rule | Ink::Background | Ink::Field | Ink::Hover | Ink::Cursor | Ink::Selection => None,
            Ink::Faint => Some(3.0),
            _ => Some(4.5),
        }
    }
}

pub static LIGHT: LazyLock<Theme> = LazyLock::new(|| crate::look::THEMES[0].theme(Appearance::Light));

pub static DARK: LazyLock<Theme> = LazyLock::new(|| crate::look::THEMES[0].theme(Appearance::Dark));

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

/// A kind of color blindness, the whole of it, as the taste laws judge a screen through it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sight {
    /// No red cones.
    Protanopia,
    /// No green cones.
    Deuteranopia,
    /// No blue cones.
    Tritanopia,
}

impl Sight {
    pub const ALL: [Sight; 3] = [Sight::Protanopia, Sight::Deuteranopia, Sight::Tritanopia];

    pub fn name(self) -> &'static str {
        match self {
            Sight::Protanopia => "protanopia",
            Sight::Deuteranopia => "deuteranopia",
            Sight::Tritanopia => "tritanopia",
        }
    }

    /// A color as someone with this sight sees it, by the model of Machado, Oliveira and Fernandes (2009) at full severity, worked in linear light.
    pub fn sees(self, c: Color) -> Color {
        let m: [[f32; 3]; 3] = match self {
            Sight::Protanopia => [[0.152_286, 1.052_583, -0.204_868], [0.114_503, 0.786_281, 0.099_216], [-0.003_882, -0.048_116, 1.051_998]],
            Sight::Deuteranopia => [[0.367_322, 0.860_646, -0.227_968], [0.280_085, 0.672_501, 0.047_413], [-0.011_820, 0.042_940, 0.968_881]],
            Sight::Tritanopia => [[1.255_528, -0.076_749, -0.178_779], [-0.078_411, 0.930_809, 0.147_602], [0.004_733, 0.691_367, 0.303_900]],
        };
        let decode = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        let encode = |v: f32| {
            let v = v.clamp(0.0, 1.0);
            if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
        };
        let rgb = [decode(c[0]), decode(c[1]), decode(c[2])];
        let row = |r: [f32; 3]| encode(r[0] * rgb[0] + r[1] * rgb[1] + r[2] * rgb[2]);
        [row(m[0]), row(m[1]), row(m[2]), c[3]]
    }
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

impl ledger::Field for Ink {
    ledger::register!(Ink);

    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn value(&self) -> ledger::Value {
        ledger::Value::Variant(format!("{self:?}"), Box::new(ledger::Value::Unit))
    }

    fn from_value(value: &ledger::Value) -> Option<Self> {
        let ledger::Value::Variant(name, _) = value else { return None };
        Ink::ALL.into_iter().find(|ink| format!("{ink:?}") == *name)
    }
}

impl ledger::Document for Ink {}
