use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use crate::ink::{Appearance, Ink, Theme, contrast, oklch};
use crate::metrics::Metrics;

/// A color as OKLCH: lightness from 0 to 1, chroma, and hue in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

impl fmt::Display for Oklch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let n = |v: f32, places: i32| {
            let k = 10f32.powi(places);
            (v * k).round() / k
        };
        write!(f, "oklch({} {} {})", n(self.l, 3), n(self.c, 3), n(self.h, 1))
    }
}

/// A theme as its file has it: a name and every ink in both appearances.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeFile {
    pub name: String,
    pub light: [Oklch; 14],
    pub dark: [Oklch; 14],
}

/// A style's values: how things are drawn, never where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub lines: Lines,
    pub shadow: f32,
    pub check: CheckMark,
}

/// A style as its file has it: a name and its values.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleFile {
    pub name: String,
    pub style: Style,
}

/// The weight of rules and the edges of boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lines {
    Thin,
    Regular,
    Heavy,
}

/// What a ticked box shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckMark {
    Box,
    Tick,
}

/// Which appearance to draw in: the system's, or one the user chose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Choice {
    #[default]
    System,
    Light,
    Dark,
}

/// What is wrong with a theme or style file, and on which line.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub line: Option<usize>,
    pub says: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.says),
            None => write!(f, "{}", self.says),
        }
    }
}

fn problem(line: usize, says: impl Into<String>) -> Problem {
    Problem { line: Some(line), says: says.into() }
}

fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())).filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
}

fn pair(n: usize, line: &str) -> Result<(&str, &str), Problem> {
    let (key, value) = line.split_once('=').ok_or_else(|| problem(n, format!("“{line}” should be a name, an equals sign and a value")))?;
    Ok((key.trim(), value.trim()))
}

fn parse_oklch(n: usize, value: &str) -> Result<Oklch, Problem> {
    let inner = value.strip_prefix("oklch(").and_then(|v| v.strip_suffix(')')).ok_or_else(|| problem(n, format!("“{value}” should be oklch(lightness chroma hue)")))?;
    let parts: Vec<f32> = inner.split_whitespace().map(str::parse).collect::<Result<_, _>>().map_err(|_| problem(n, format!("“{inner}” should be three numbers")))?;
    let [l, c, h] = parts[..] else { return Err(problem(n, format!("“{inner}” should be three numbers: lightness, chroma and hue"))) };
    if !(0.0..=1.0).contains(&l) {
        return Err(problem(n, format!("lightness {l} should be between 0 and 1")));
    }
    if !(0.0..=0.5).contains(&c) {
        return Err(problem(n, format!("chroma {c} should be between 0 and 0.5")));
    }
    if !h.is_finite() {
        return Err(problem(n, "the hue should be a number of degrees"));
    }
    Ok(Oklch { l, c, h: h.rem_euclid(360.0) })
}

impl ThemeFile {
    /// Reads a theme file, or says what is wrong with it. A theme whose text inks don't reach their contrast is refused too.
    pub fn parse(text: &str) -> Result<ThemeFile, Problem> {
        let mut name = None;
        let mut section = None;
        let mut light: [Option<Oklch>; 14] = [None; 14];
        let mut dark: [Option<Oklch>; 14] = [None; 14];
        for (n, line) in lines(text) {
            match line {
                "[light]" => section = Some(Appearance::Light),
                "[dark]" => section = Some(Appearance::Dark),
                _ if line.starts_with('[') => return Err(problem(n, format!("{line} isn't a part of a theme; the parts are [light] and [dark]"))),
                _ => {
                    let (key, value) = pair(n, line)?;
                    match (section, key) {
                        (None, "name") => name = Some(value.to_string()),
                        (None, _) => return Err(problem(n, format!("“{key}” comes before [light] or [dark]; only the name does"))),
                        (Some(appearance), _) => {
                            let i = Ink::ALL.iter().position(|ink| ink.name() == key).ok_or_else(|| problem(n, format!("“{key}” isn't one of the inks")))?;
                            let slot = if appearance == Appearance::Light { &mut light[i] } else { &mut dark[i] };
                            if slot.is_some() {
                                return Err(problem(n, format!("{key} is given twice")));
                            }
                            *slot = Some(parse_oklch(n, value)?);
                        }
                    }
                }
            }
        }
        let name = name.filter(|n| !n.is_empty()).ok_or(Problem { line: None, says: "the theme has no name".into() })?;
        let whole = |inks: [Option<Oklch>; 14], look: &str| -> Result<[Oklch; 14], Problem> {
            let mut out = [Oklch { l: 0.0, c: 0.0, h: 0.0 }; 14];
            for (i, ink) in inks.iter().enumerate() {
                out[i] = ink.ok_or_else(|| Problem { line: None, says: format!("{look} has no {}", Ink::ALL[i].name()) })?;
            }
            Ok(out)
        };
        let file = ThemeFile { name, light: whole(light, "[light]")?, dark: whole(dark, "[dark]")? };
        file.check()?;
        Ok(file)
    }

    /// The theme's file, which reads back as the same theme.
    pub fn print(&self) -> String {
        let mut out = format!("name = {}\n", self.name);
        for (header, inks) in [("[light]", &self.light), ("[dark]", &self.dark)] {
            out.push_str(&format!("\n{header}\n"));
            for (ink, color) in Ink::ALL.iter().zip(inks) {
                out.push_str(&format!("{} = {color}\n", ink.name()));
            }
        }
        out
    }

    pub fn get(&self, appearance: Appearance, ink: Ink) -> Oklch {
        let i = Ink::ALL.iter().position(|x| *x == ink).expect("every ink is in ALL");
        if appearance == Appearance::Light { self.light[i] } else { self.dark[i] }
    }

    pub fn set(&mut self, appearance: Appearance, ink: Ink, color: Oklch) {
        let i = Ink::ALL.iter().position(|x| *x == ink).expect("every ink is in ALL");
        if appearance == Appearance::Light {
            self.light[i] = color;
        } else {
            self.dark[i] = color;
        }
    }

    /// The theme's colors in one appearance.
    pub fn theme(&self, appearance: Appearance) -> Theme {
        let c = |ink| {
            let o = self.get(appearance, ink);
            oklch(o.l, o.c, o.h)
        };
        Theme {
            bg: c(Ink::Background),
            ink: c(Ink::Ink),
            muted: c(Ink::Muted),
            faint: c(Ink::Faint),
            rule: c(Ink::Rule),
            cursor: c(Ink::Cursor),
            hover: c(Ink::Hover),
            selection: c(Ink::Selection),
            field: c(Ink::Field),
            link: c(Ink::Link),
            visited: c(Ink::Visited),
            danger: c(Ink::Danger),
            warn: c(Ink::Warn),
            ok: c(Ink::Ok),
        }
    }

    /// The first ink in either appearance that doesn't reach its contrast on the background, said in words.
    pub fn check(&self) -> Result<(), Problem> {
        for (appearance, look) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
            let theme = self.theme(appearance);
            for ink in Ink::ALL {
                let Some(needs) = ink.needs() else { continue };
                let has = contrast(theme.get(ink), theme.bg);
                if has < needs {
                    return Err(Problem { line: None, says: format!("{} reads at {has:.1}:1 on the {look} background and needs {needs}:1", ink.name()) });
                }
            }
        }
        Ok(())
    }
}

impl Style {
    /// How thick rules and the edges of boxes are, in pixels.
    pub fn line(&self, m: Metrics) -> f32 {
        let hair = m.hair();
        match self.lines {
            Lines::Thin => (hair / 2.0).round().max(1.0),
            Lines::Regular => hair,
            Lines::Heavy => hair * 2.0,
        }
    }

    /// How deep the hard shadow under what you can act on is, in pixels; none when the style has no shadows.
    pub fn shadow(&self, m: Metrics) -> f32 {
        if self.shadow <= 0.0 { 0.0 } else { (m.cell_h * self.shadow).round().max(m.hair() * 2.0) }
    }
}

impl Default for Style {
    fn default() -> Style {
        STYLES[0].style
    }
}

impl StyleFile {
    /// Reads a style file, or says what is wrong with it.
    pub fn parse(text: &str) -> Result<StyleFile, Problem> {
        let (mut name, mut lines_, mut shadow, mut check) = (None, None, None, None);
        for (n, line) in lines(text) {
            let (key, value) = pair(n, line)?;
            match key {
                "name" => name = Some(value.to_string()),
                "lines" => {
                    lines_ = Some(match value {
                        "thin" => Lines::Thin,
                        "regular" => Lines::Regular,
                        "heavy" => Lines::Heavy,
                        _ => return Err(problem(n, format!("lines is “{value}”, and should be thin, regular or heavy"))),
                    })
                }
                "shadow" => {
                    let v: f32 = value.parse().map_err(|_| problem(n, format!("shadow is “{value}”, and should be a number")))?;
                    if !(0.0..=0.5).contains(&v) {
                        return Err(problem(n, format!("shadow is {v}, and should be between 0 and 0.5 of a row")));
                    }
                    shadow = Some(v);
                }
                "check" => {
                    check = Some(match value {
                        "box" => CheckMark::Box,
                        "tick" => CheckMark::Tick,
                        _ => return Err(problem(n, format!("check is “{value}”, and should be box or tick"))),
                    })
                }
                _ => return Err(problem(n, format!("“{key}” isn't a value of a style; they are name, lines, shadow and check"))),
            }
        }
        let missing = |what: &str| Problem { line: None, says: format!("the style has no {what}") };
        Ok(StyleFile {
            name: name.filter(|n| !n.is_empty()).ok_or_else(|| missing("name"))?,
            style: Style { lines: lines_.ok_or_else(|| missing("lines"))?, shadow: shadow.ok_or_else(|| missing("shadow"))?, check: check.ok_or_else(|| missing("check"))? },
        })
    }

    /// The style's file, which reads back as the same style.
    pub fn print(&self) -> String {
        let lines = match self.style.lines {
            Lines::Thin => "thin",
            Lines::Regular => "regular",
            Lines::Heavy => "heavy",
        };
        let check = match self.style.check {
            CheckMark::Box => "box",
            CheckMark::Tick => "tick",
        };
        format!("name = {}\nlines = {lines}\nshadow = {}\ncheck = {check}\n", self.name, self.style.shadow)
    }
}

/// The themes Graticule ships, the first of them the default.
pub static THEMES: LazyLock<Vec<ThemeFile>> = LazyLock::new(|| {
    [include_str!("../looks/graticule.theme"), include_str!("../looks/contrast.theme")]
        .into_iter()
        .map(|t| ThemeFile::parse(t).expect("the built-in themes read and pass the contrast law"))
        .collect()
});

/// The styles Graticule ships, the first of them the default.
pub static STYLES: LazyLock<Vec<StyleFile>> = LazyLock::new(|| {
    [include_str!("../looks/graticule.style"), include_str!("../looks/flat.style")]
        .into_iter()
        .map(|s| StyleFile::parse(s).expect("the built-in styles read"))
        .collect()
});

/// Everything a frame is drawn in: the theme's colors in the appearance shown, and the style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub theme: Theme,
    pub style: Style,
    pub appearance: Appearance,
}

impl Look {
    /// Graticule's own theme and style in an appearance.
    pub fn of(appearance: Appearance) -> Look {
        Look { theme: *Theme::of(appearance), style: Style::default(), appearance }
    }
}

/// A change to the look, asked for by a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    Appearance(Choice),
    NextTheme,
    NextStyle,
}

/// Every theme and style there is, the built-in ones and the user's own files, and which of them the user chose.
#[derive(Clone, Debug, PartialEq)]
pub struct Looks {
    pub themes: Vec<ThemeFile>,
    pub styles: Vec<StyleFile>,
    pub theme: usize,
    pub style: usize,
    pub appearance: Choice,
    /// Files that couldn't be read, each with what is wrong.
    pub problems: Vec<String>,
}

impl Default for Looks {
    fn default() -> Looks {
        Looks { themes: THEMES.clone(), styles: STYLES.clone(), theme: 0, style: 0, appearance: Choice::System, problems: Vec::new() }
    }
}

impl Looks {
    /// The built-in looks, then every `.theme` and `.style` file in `folder` (one with a built-in one's name replaces it), and the choice saved in `look.txt`.
    pub fn load(folder: &Path) -> Looks {
        let mut looks = Looks::default();
        let mut files: Vec<_> = std::fs::read_dir(folder).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for file in files {
            let Some(kind) = file.extension().and_then(|e| e.to_str()) else { continue };
            if kind != "theme" && kind != "style" {
                continue;
            }
            let shown = file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let read = std::fs::read_to_string(&file).map_err(|e| Problem { line: None, says: e.to_string() });
            let added = match kind {
                "theme" => read.and_then(|t| ThemeFile::parse(&t)).map(|t| match looks.themes.iter().position(|x| x.name == t.name) {
                    Some(i) => looks.themes[i] = t,
                    None => looks.themes.push(t),
                }),
                _ => read.and_then(|s| StyleFile::parse(&s)).map(|s| match looks.styles.iter().position(|x| x.name == s.name) {
                    Some(i) => looks.styles[i] = s,
                    None => looks.styles.push(s),
                }),
            };
            if let Err(p) = added {
                looks.problems.push(format!("{shown}: {p}"));
            }
        }
        if let Ok(chosen) = std::fs::read_to_string(folder.join("look.txt")) {
            looks.choose(&chosen);
        }
        looks
    }

    fn choose(&mut self, chosen: &str) {
        for (_, line) in lines(chosen) {
            let Ok((key, value)) = pair(0, line) else { continue };
            match key {
                "theme" => self.theme = self.themes.iter().position(|t| t.name == value).unwrap_or(self.theme),
                "style" => self.style = self.styles.iter().position(|s| s.name == value).unwrap_or(self.style),
                "appearance" => {
                    self.appearance = match value {
                        "light" => Choice::Light,
                        "dark" => Choice::Dark,
                        _ => Choice::System,
                    }
                }
                _ => {}
            }
        }
    }

    /// The choice, as `look.txt` keeps it.
    pub fn chosen(&self) -> String {
        let appearance = match self.appearance {
            Choice::System => "system",
            Choice::Light => "light",
            Choice::Dark => "dark",
        };
        format!("theme = {}\nstyle = {}\nappearance = {appearance}\n", self.themes[self.theme].name, self.styles[self.style].name)
    }

    /// The look to draw in, given the system's appearance.
    pub fn look(&self, system: Appearance) -> Look {
        let appearance = match self.appearance {
            Choice::System => system,
            Choice::Light => Appearance::Light,
            Choice::Dark => Appearance::Dark,
        };
        Look { theme: self.themes[self.theme].theme(appearance), style: self.styles[self.style].style, appearance }
    }

    pub fn switch(&mut self, s: Switch) {
        match s {
            Switch::Appearance(c) => self.appearance = c,
            Switch::NextTheme => self.theme = (self.theme + 1) % self.themes.len(),
            Switch::NextStyle => self.style = (self.style + 1) % self.styles.len(),
        }
    }
}

/// What a face needs to know of the looks to say which switches make sense: the appearance chosen, and how many themes and styles there are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offered {
    pub appearance: Choice,
    pub themes: usize,
    pub styles: usize,
}

impl Looks {
    pub fn offered(&self) -> Offered {
        Offered { appearance: self.appearance, themes: self.themes.len(), styles: self.styles.len() }
    }
}

/// Where the user's own themes and styles live, and the choice of look, shared by every Graticule app.
pub fn folder() -> std::path::PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Graticule").join("looks")
}
