use std::collections::HashMap;

use crate::font::Font;
use crate::hyphen::Hyphenation;

/// Makes typed punctuation the typesetter's: straight quotes curl to open or close by what stands before them, so an apostrophe closes; `---` becomes an em dash, `--` an en dash, a hyphen with a space either side a spaced en dash, and `...` an ellipsis.
pub fn punctuate(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let before = out.chars().next_back();
        let opens = before.is_none_or(|b| b.is_whitespace() || "([{\u{2014}\u{2013}\u{201c}\u{2018}".contains(b));
        let run = |of: char| chars[i..].iter().take_while(|&&x| x == of).count();
        match c {
            '"' => out.push(if opens { '\u{201c}' } else { '\u{201d}' }),
            '\'' => out.push(if opens { '\u{2018}' } else { '\u{2019}' }),
            '-' if run('-') >= 3 => {
                out.push('\u{2014}');
                i += 2;
            }
            '-' if run('-') == 2 => {
                out.push('\u{2013}');
                i += 1;
            }
            '-' if before == Some(' ') && chars.get(i + 1) == Some(&' ') => out.push('\u{2013}'),
            '.' if run('.') == 3 => {
                out.push('\u{2026}');
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// The paper a document is set on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Paper {
    #[default]
    A4,
    Letter,
}

impl Paper {
    /// Its width and height in points, of which an inch holds 72.
    pub fn size(self) -> (f32, f32) {
        match self {
            Paper::A4 => (595.276, 841.89),
            Paper::Letter => (612.0, 792.0),
        }
    }
}

/// How a text is set: on what paper, the type's size and the distance from one line to the next, and the margin all round, all in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    pub paper: Paper,
    pub size: f32,
    pub leading: f32,
    pub margin: f32,
}

impl Default for Setting {
    fn default() -> Self {
        Setting { paper: Paper::A4, size: 11.0, leading: 15.0, margin: 72.0 }
    }
}

/// A glyph in its place: the character it stands for, the font's glyph and how far it is from the page's left edge, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub c: char,
    pub id: u16,
    pub x: f32,
}

/// A line set on a page: how far its baseline is from the top of the page, in points, and its glyphs from left to right.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub baseline: f32,
    pub glyphs: Vec<Glyph>,
}

/// A page of set lines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub lines: Vec<Line>,
}

/// A text set in a font: its pages, ready to draw or to write as a PDF at real size.
#[derive(Clone, Debug)]
pub struct Typeset {
    pub font: Font,
    pub setting: Setting,
    pub pages: Vec<Page>,
}

const INFINITE: f32 = 10000.0;
const HYPHEN_COST: f32 = 50.0;
const FLAGGED: f32 = 3000.0;
const MISFIT: f32 = 3000.0;

#[derive(Clone, Copy, Debug)]
enum Item {
    Box { width: f32, from: usize, to: usize },
    Glue { width: f32, stretch: f32, shrink: f32 },
    Penalty { width: f32, cost: f32, flagged: bool },
}

struct Node {
    at: usize,
    fitness: usize,
    flagged: bool,
    width: f32,
    stretch: f32,
    shrink: f32,
    demerits: f32,
    ratio: f32,
    prev: Option<usize>,
}

struct Metrics<'f> {
    font: &'f Font,
    size: f32,
    known: HashMap<char, (u16, f32)>,
}

impl Metrics<'_> {
    fn of(&mut self, c: char) -> (u16, f32) {
        let (font, size) = (self.font, self.size);
        *self.known.entry(c).or_insert_with(|| {
            let id = font.glyph(c);
            (id, font.advance(id) * size)
        })
    }
}

/// Sets `text` in `font`: blank lines part paragraphs and the lines within one run together; punctuation is made the typesetter's; each paragraph is broken into lines as a whole, choosing the breaks that leave the evenest spaces, hyphenating where `hyphenation` allows; lines are justified but for each paragraph's last, and flow onto as many pages as they need.
pub fn set(text: &str, font: &Font, setting: Setting, hyphenation: Option<&Hyphenation>) -> Typeset {
    let (width, height) = setting.paper.size();
    let measure = (width - 2.0 * setting.margin).max(setting.size);
    let bottom = height - setting.margin;
    let first = setting.margin + font.ascent * setting.size;
    let mut metrics = Metrics { font, size: setting.size, known: HashMap::new() };
    let mut pages = vec![Page::default()];
    let mut baseline = first;
    for paragraph in paragraphs(text) {
        let chars: Vec<char> = punctuate(&paragraph).chars().collect();
        let items = items(&chars, &mut metrics, hyphenation);
        let breaks = [1.0, 2.0, 10.0].into_iter().find_map(|tolerance| breaks(&items, measure, tolerance, false)).or_else(|| breaks(&items, measure, f32::INFINITY, true)).unwrap_or_default();
        let mut start = 0;
        for (at, ratio) in breaks {
            if baseline > bottom && pages.last().is_some_and(|p| !p.lines.is_empty()) {
                pages.push(Page::default());
                baseline = first;
            }
            let line = place(&items[start..at], &items[at], &chars, &mut metrics, setting.margin, ratio, baseline);
            if let Some(page) = pages.last_mut() {
                page.lines.push(line);
            }
            baseline += setting.leading;
            start = after(&items, at);
        }
        baseline += setting.leading / 2.0;
    }
    Typeset { font: font.clone(), setting, pages }
}

fn paragraphs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn items(chars: &[char], metrics: &mut Metrics, hyphenation: Option<&Hyphenation>) -> Vec<Item> {
    let space = metrics.of(' ').1;
    let hyphen = metrics.of('-').1;
    let mut items = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && !items.is_empty() {
                if matches!(chars[i], '\u{2013}' | '\u{2014}') && chars.get(i + 1).is_none_or(|c| c.is_whitespace()) {
                    items.push(Item::Penalty { width: 0.0, cost: INFINITE, flagged: false });
                }
                items.push(Item::Glue { width: space, stretch: space / 2.0, shrink: space / 3.0 });
            }
            continue;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let mut cuts: Vec<(usize, bool)> = (start + 1..i).filter(|&k| matches!(chars[k - 1], '-' | '\u{2013}' | '\u{2014}')).map(|k| (k, false)).collect();
        if let Some(hyphenation) = hyphenation {
            let core = letters(&chars[start..i]);
            let word: String = chars[start + core.start..start + core.end].iter().collect();
            cuts.extend(hyphenation.points(&word).into_iter().map(|k| (start + core.start + k, true)));
            cuts.sort_unstable();
        }
        let mut from = start;
        for (cut, hyphenated) in cuts.into_iter().chain([(i, false)]) {
            let width = (from..cut).map(|k| metrics.of(chars[k]).1).sum();
            items.push(Item::Box { width, from, to: cut });
            if cut < i {
                items.push(Item::Penalty { width: if hyphenated { hyphen } else { 0.0 }, cost: HYPHEN_COST, flagged: true });
            }
            from = cut;
        }
    }
    items.push(Item::Penalty { width: 0.0, cost: INFINITE, flagged: false });
    items.push(Item::Glue { width: 0.0, stretch: 1.0e6, shrink: 0.0 });
    items.push(Item::Penalty { width: 0.0, cost: -INFINITE, flagged: true });
    items
}

fn letters(word: &[char]) -> std::ops::Range<usize> {
    let start = word.iter().position(|c| c.is_alphabetic()).unwrap_or(word.len());
    let end = word.iter().rposition(|c| c.is_alphabetic()).map_or(start, |e| e + 1);
    let end = (start..end).find(|&k| !word[k].is_alphabetic()).unwrap_or(end);
    start..end
}

fn breaks(items: &[Item], measure: f32, tolerance: f32, emergency: bool) -> Option<Vec<(usize, f32)>> {
    let mut nodes = vec![Node { at: 0, fitness: 1, flagged: false, width: 0.0, stretch: 0.0, shrink: 0.0, demerits: 0.0, ratio: 0.0, prev: None }];
    let mut active = vec![0];
    let (mut width, mut stretch, mut shrink) = (0.0, 0.0, 0.0);
    for (i, item) in items.iter().enumerate() {
        let legal = match *item {
            Item::Box { .. } => false,
            Item::Glue { .. } => i > 0 && matches!(items[i - 1], Item::Box { .. }),
            Item::Penalty { cost, .. } => cost < INFINITE,
        };
        if legal {
            let (extra, cost, flagged) = match *item {
                Item::Penalty { width, cost, flagged } => (width, cost, flagged),
                _ => (0.0, 0.0, false),
            };
            let mut best: [Option<(f32, usize, f32)>; 4] = [None; 4];
            let mut kept = Vec::with_capacity(active.len());
            let mut dropped: Option<usize> = None;
            for &a in &active {
                let n = &nodes[a];
                let length = width - n.width + extra;
                let ratio = if length < measure {
                    let s = stretch - n.stretch;
                    if s > 0.0 { (measure - length) / s } else { INFINITE }
                } else if length > measure {
                    let s = shrink - n.shrink;
                    if s > 0.0 { (measure - length) / s } else { -INFINITE }
                } else {
                    0.0
                };
                if ratio < -1.0 || cost <= -INFINITE {
                    if dropped.is_none_or(|d| n.demerits < nodes[d].demerits) {
                        dropped = Some(a);
                    }
                } else {
                    kept.push(a);
                }
                if (-1.0..=tolerance).contains(&ratio) {
                    let badness = 100.0 * ratio.abs().powi(3);
                    let mut demerits = if cost >= 0.0 {
                        (1.0 + badness + cost).powi(2)
                    } else if cost > -INFINITE {
                        (1.0 + badness).powi(2) - cost * cost
                    } else {
                        (1.0 + badness).powi(2)
                    };
                    if flagged && n.flagged && cost > -INFINITE {
                        demerits += FLAGGED;
                    }
                    let fitness = fitness(ratio);
                    if fitness.abs_diff(n.fitness) > 1 {
                        demerits += MISFIT;
                    }
                    let total = n.demerits + demerits;
                    if best[fitness].is_none_or(|b| total < b.0) {
                        best[fitness] = Some((total, a, ratio));
                    }
                }
            }
            active = kept;
            if emergency
                && active.is_empty()
                && best.iter().all(Option::is_none)
                && let Some(d) = dropped
            {
                best[0] = Some((nodes[d].demerits + 1.0e6, d, -1.0));
            }
            if best.iter().any(Option::is_some) {
                let (mut w, mut st, mut sh) = (width, stretch, shrink);
                for (j, next) in items.iter().enumerate().skip(i) {
                    match *next {
                        Item::Box { .. } => break,
                        Item::Glue { width, stretch, shrink } => (w, st, sh) = (w + width, st + stretch, sh + shrink),
                        Item::Penalty { cost, .. } if cost <= -INFINITE && j > i => break,
                        Item::Penalty { .. } => {}
                    }
                }
                for (fitness, found) in best.into_iter().enumerate() {
                    if let Some((demerits, from, ratio)) = found {
                        nodes.push(Node { at: i, fitness, flagged, width: w, stretch: st, shrink: sh, demerits, ratio, prev: Some(from) });
                        active.push(nodes.len() - 1);
                    }
                }
            }
            if active.is_empty() {
                return None;
            }
        }
        match *item {
            Item::Box { width: w, .. } => width += w,
            Item::Glue { width: w, stretch: st, shrink: sh } => (width, stretch, shrink) = (width + w, stretch + st, shrink + sh),
            Item::Penalty { .. } => {}
        }
    }
    let end = items.len().checked_sub(1)?;
    let mut at = active.into_iter().filter(|&a| nodes[a].at == end).min_by(|&a, &b| nodes[a].demerits.total_cmp(&nodes[b].demerits));
    let mut out = Vec::new();
    while let Some(n) = at.map(|a| &nodes[a]) {
        if n.prev.is_none() {
            break;
        }
        out.push((n.at, n.ratio));
        at = n.prev;
    }
    out.reverse();
    (!out.is_empty()).then_some(out)
}

fn fitness(ratio: f32) -> usize {
    match ratio {
        r if r < -0.5 => 0,
        r if r <= 0.5 => 1,
        r if r <= 1.0 => 2,
        _ => 3,
    }
}

fn after(items: &[Item], at: usize) -> usize {
    let mut k = at + 1;
    while k < items.len() && !matches!(items[k], Item::Box { .. }) {
        if matches!(items[k], Item::Penalty { cost, .. } if cost <= -INFINITE) {
            break;
        }
        k += 1;
    }
    k
}

fn place(items: &[Item], end: &Item, chars: &[char], metrics: &mut Metrics, margin: f32, ratio: f32, baseline: f32) -> Line {
    let mut x = margin;
    let mut glyphs = Vec::new();
    for item in items {
        match *item {
            Item::Box { from, to, .. } => {
                for &c in &chars[from..to] {
                    let (id, advance) = metrics.of(c);
                    glyphs.push(Glyph { c, id, x });
                    x += advance;
                }
            }
            Item::Glue { width, stretch, shrink } => x += width + if ratio >= 0.0 { ratio * stretch } else { ratio * shrink },
            Item::Penalty { .. } => {}
        }
    }
    if let Item::Penalty { width, flagged: true, .. } = *end
        && width > 0.0
    {
        let (id, _) = metrics.of('-');
        glyphs.push(Glyph { c: '-', id, x });
    }
    Line { baseline, glyphs }
}
