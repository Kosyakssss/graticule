use graticule_macros::Facts;
use kernel::ink::{Appearance, Theme, contrast, oklch};
use kernel::look::{CheckMark, Lines, Look, Oklch, Style, StyleFile, ThemeFile};
use kernel::offscreen::Offscreen;
use kernel::runner::Runner;
use kernel::{App, Ink, Message};
use ledger::{Key, List};

/// An ink's color in one appearance as the style editor keeps it: lightness and chroma in thousandths, hue in tenths of a degree, so every step lands on a number the theme file writes exactly.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shade {
    pub l: u32,
    pub c: u32,
    pub h: u32,
}

impl Shade {
    fn of(o: Oklch) -> Shade {
        Shade { l: (o.l * 1000.0).round() as u32, c: (o.c * 1000.0).round() as u32, h: (o.h * 10.0).round() as u32 % 3600 }
    }

    pub fn oklch(self) -> Oklch {
        Oklch { l: self.l as f32 / 1000.0, c: self.c as f32 / 1000.0, h: self.h as f32 / 10.0 }
    }

    fn color(self) -> kernel::ink::Color {
        let o = self.oklch();
        oklch(o.l, o.c, o.h)
    }
}

/// One ink of the theme being edited, in both appearances.
#[derive(Facts)]
pub struct Mix {
    #[key]
    pub id: Key<Mix>,
    pub ink: Ink,
    pub light: Shade,
    pub dark: Shade,
}

/// Which appearance the editor's changes go to.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Light,
    Dark,
}

/// The weight of lines, as the editor keeps it.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Weight {
    Thin,
    #[default]
    Regular,
    Heavy,
}

/// The check mark, as the editor keeps it.
#[derive(Facts, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mark {
    #[default]
    Box,
    Tick,
}

/// How many of the app's screens draw in an ink.
#[derive(Facts)]
pub struct Reach {
    #[key]
    id: Key<Reach>,
    pub ink: Ink,
    pub screens: u32,
}

/// What counting the screens found: how many screens draw in each ink, out of how many.
#[derive(Facts, Default)]
pub struct Reaches {
    #[session]
    pub inks: Vec<Reach>,
    pub screens: u32,
}

/// One step of a change to an ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nudge {
    Lighter,
    Darker,
    MoreColor,
    LessColor,
    HueForward,
    HueBack,
}

impl Nudge {
    fn apply(self, s: Shade) -> Shade {
        match self {
            Nudge::Lighter => Shade { l: (s.l + 10).min(1000), ..s },
            Nudge::Darker => Shade { l: s.l.saturating_sub(10), ..s },
            Nudge::MoreColor => Shade { c: (s.c + 5).min(370), ..s },
            Nudge::LessColor => Shade { c: s.c.saturating_sub(5), ..s },
            Nudge::HueForward => Shade { h: (s.h + 50) % 3600, ..s },
            Nudge::HueBack => Shade { h: (s.h + 3600 - 50) % 3600, ..s },
        }
    }

    fn limit(self, ink: Ink) -> String {
        let what = match self {
            Nudge::Lighter => "as light as it goes",
            Nudge::Darker => "as dark as it goes",
            Nudge::MoreColor => "as colorful as it goes",
            Nudge::LessColor => "gray already",
            Nudge::HueForward | Nudge::HueBack => "gray, so it has no hue to turn",
        };
        format!("{} is {what}", capital(ink.name()))
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// The theme the editor starts from: Graticule's own, ink by ink.
pub fn mixes() -> List<Mix> {
    let file = &kernel::look::THEMES[0];
    Ink::ALL
        .into_iter()
        .zip(1..)
        .map(|(ink, n)| Mix { id: Key::fixed(n), ink, light: Shade::of(file.get(Appearance::Light, ink)), dark: Shade::of(file.get(Appearance::Dark, ink)) })
        .collect()
}

pub fn shade(inks: &List<Mix>, ink: Ink, side: Side) -> Shade {
    inks.iter().find(|m| m.ink == ink).map(|m| if side == Side::Light { m.light } else { m.dark }).unwrap_or_default()
}

/// The ink after one step, or why that step can't be taken: it would go past the end, or leave some text unreadable.
pub fn nudged(inks: &List<Mix>, ink: Ink, side: Side, by: Nudge) -> Result<Shade, Message> {
    let now = shade(inks, ink, side);
    let next = by.apply(now);
    if next == now || (now.c == 0 && matches!(by, Nudge::HueForward | Nudge::HueBack)) {
        return Err(Message::plain(by.limit(ink)));
    }
    let color = |i: Ink| if i == ink { next.color() } else { shade(inks, i, side).color() };
    let bg = color(Ink::Background);
    let judged: Vec<Ink> = if ink == Ink::Background { Ink::ALL.to_vec() } else { vec![ink] };
    for i in judged {
        let Some(needs) = i.needs() else { continue };
        let has = contrast(color(i), bg);
        if has < needs {
            return Err(Message::plain(format!("That would leave {} at {has:.1}:1 on the background, and it needs {needs}:1", i.name())));
        }
    }
    Ok(next)
}

pub fn theme_file(inks: &List<Mix>) -> ThemeFile {
    let mut file = kernel::look::THEMES[0].clone();
    for m in inks.iter() {
        file.set(Appearance::Light, m.ink, m.light.oklch());
        file.set(Appearance::Dark, m.ink, m.dark.oklch());
    }
    file
}

pub fn style(lines: Weight, shadow: u32, check: Mark) -> Style {
    let lines = match lines {
        Weight::Thin => Lines::Thin,
        Weight::Regular => Lines::Regular,
        Weight::Heavy => Lines::Heavy,
    };
    let check = match check {
        Mark::Box => CheckMark::Box,
        Mark::Tick => CheckMark::Tick,
    };
    Style { lines, shadow: shadow as f32 / 100.0, check }
}

pub fn style_file(lines: Weight, shadow: u32, check: Mark) -> StyleFile {
    StyleFile { name: kernel::look::STYLES[0].name.clone(), style: style(lines, shadow, check) }
}

/// The values of Graticule's own style, as the editor keeps them.
pub fn start() -> (Weight, u32, Mark) {
    let s = kernel::look::STYLES[0].style;
    let lines = match s.lines {
        Lines::Thin => Weight::Thin,
        Lines::Regular => Weight::Regular,
        Lines::Heavy => Weight::Heavy,
    };
    let check = match s.check {
        CheckMark::Box => Mark::Box,
        CheckMark::Tick => Mark::Tick,
    };
    (lines, (s.shadow * 100.0).round() as u32, check)
}

/// A theme in which every ink is a color no other ink has, so the colors a screen is painted in say which inks it uses.
fn coded() -> Theme {
    let code = |i: usize| [(i + 1) as f32 / 32.0, 0.25, 0.75, 1.0];
    let at = |ink: Ink| code(Ink::ALL.iter().position(|x| *x == ink).unwrap_or_default());
    Theme {
        bg: at(Ink::Background),
        ink: at(Ink::Ink),
        muted: at(Ink::Muted),
        faint: at(Ink::Faint),
        rule: at(Ink::Rule),
        cursor: at(Ink::Cursor),
        hover: at(Ink::Hover),
        selection: at(Ink::Selection),
        field: at(Ink::Field),
        link: at(Ink::Link),
        visited: at(Ink::Visited),
        danger: at(Ink::Danger),
        warn: at(Ink::Warn),
        ok: at(Ink::Ok),
    }
}

/// Paints every named state of the app once in the coded theme and counts, for each ink, the screens drawn in it. The background is behind every screen.
pub fn reach<A: App>() -> Reaches {
    let states = A::states().list;
    let mut screen = Offscreen::new(1920, 1280, 2.0);
    let theme = coded();
    let look = Look { theme, ..Look::of(Appearance::Light) };
    let mut counts = vec![0u32; Ink::ALL.len()];
    for (_, facts) in states.iter() {
        let mut runner = Runner::new(facts.clone());
        screen.render_face(look, &mut runner);
        let mut used = vec![false; Ink::ALL.len()];
        used[0] = true;
        for q in screen.quads() {
            for (i, ink) in Ink::ALL.into_iter().enumerate() {
                let c = theme.get(ink);
                if (q.color[0] - c[0]).abs() < 1e-4 && (q.color[1] - c[1]).abs() < 1e-4 && (q.color[2] - c[2]).abs() < 1e-4 {
                    used[i] = true;
                }
            }
        }
        for (count, used) in counts.iter_mut().zip(used) {
            *count += used as u32;
        }
    }
    let inks = Ink::ALL.into_iter().zip(counts).zip(1..).map(|((ink, screens), n)| Reach { id: Key::fixed(n), ink, screens }).collect();
    Reaches { inks, screens: states.len() as u32 }
}
