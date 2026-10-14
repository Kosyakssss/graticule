//! EN-01 Engraver: compiles SVG files into pictures, shape data in the theme's semantic inks with a label for screen readers.
//!
//! Run it from a build script:
//!
//! ```ignore
//! let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("pictures.rs");
//! engraver::engrave("pictures".as_ref(), &out, "graticule").unwrap();
//! ```
//!
//! and `mod pictures { include!(concat!(env!("OUT_DIR"), "/pictures.rs")); }` gives one `static` per file, named after it in
//! capitals, and `ALL`, every picture by name.
//!
//! A picture is drawn in the colors of the light theme, each of which stands for an ink (see [`INKS`]); black and white
//! stand for ink and background too. The `<title>` is the picture's label. Strokes become outlines here, so their caps,
//! joins and dashes come out exactly as drawn.

use std::fmt::{self, Write};
use std::path::Path;

use usvg::tiny_skia_path::{self, PathSegment, StrokeDash};
use usvg::{FillRule, Node, Paint};

#[cfg(test)]
mod laws;

/// The colors a picture may be drawn in, and the ink each one stands for: the light theme's inks, and black and white.
pub const INKS: [(&str, [u8; 3]); 13] = [
    ("Ink", [0x1a, 0x1a, 0x1d]),
    ("Muted", [0x63, 0x63, 0x68]),
    ("Faint", [0x7f, 0x80, 0x85]),
    ("Rule", [0xb7, 0xb7, 0xba]),
    ("Link", [0x2b, 0x58, 0xb1]),
    ("Visited", [0x72, 0x41, 0xa0]),
    ("Danger", [0xa9, 0x23, 0x1e]),
    ("Warn", [0x82, 0x51, 0x00]),
    ("Ok", [0x19, 0x70, 0x37]),
    ("Field", [0xef, 0xee, 0xec]),
    ("Background", [0xf9, 0xf8, 0xf6]),
    ("Ink", [0x00, 0x00, 0x00]),
    ("Background", [0xff, 0xff, 0xff]),
];

/// How far, in steps of 0 to 255 across red, green and blue together, a color may sit from the ink it stands for.
pub const NEAR: f32 = 12.0;

/// A picture compiled from one SVG file.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub name: String,
    pub label: String,
    pub width: f32,
    pub height: f32,
    pub layers: Vec<Layer>,
}

/// One filled shape of a picture, painted in one ink, in the picture's own coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub ink: &'static str,
    pub path: String,
    pub even_odd: bool,
}

/// A file made into a picture, or why it wasn't.
pub type Compiled = Result<Picture, Refusal>;

/// Why a file was not made into a picture.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    NoLabel,
    NotInk { color: [u8; 3], nearest: &'static str },
    NotSolid(&'static str),
    Unreadable(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Refusal::NoLabel => write!(f, "it has no <title>, which every picture needs as its label for screen readers"),
            Refusal::NotInk { color: [r, g, b], nearest } => {
                write!(f, "#{r:02x}{g:02x}{b:02x} is not one of the theme's inks; the nearest is {}", nearest.to_lowercase())
            }
            Refusal::NotSolid(what) => write!(f, "it uses {what}, and a picture is drawn in whole inks"),
            Refusal::Unreadable(why) => write!(f, "it can't be read as SVG: {why}"),
        }
    }
}

/// The ink a color stands for, or none when it is not near enough to any.
pub fn ink(color: [u8; 3]) -> Result<&'static str, Refusal> {
    let distance = |rgb: [u8; 3]| color.iter().zip(rgb).map(|(a, b)| (*a as f32 - b as f32).powi(2)).sum::<f32>().sqrt();
    let (name, rgb) = INKS.iter().min_by(|a, b| distance(a.1).total_cmp(&distance(b.1))).expect("inks");
    if distance(*rgb) <= NEAR { Ok(name) } else { Err(Refusal::NotInk { color, nearest: name }) }
}

fn paint(paint: &Paint, opacity: f32) -> Result<&'static str, Refusal> {
    if opacity < 1.0 {
        return Err(Refusal::NotSolid("transparency"));
    }
    match paint {
        Paint::Color(c) => ink([c.red, c.green, c.blue]),
        Paint::LinearGradient(_) | Paint::RadialGradient(_) => Err(Refusal::NotSolid("a gradient")),
        Paint::Pattern(_) => Err(Refusal::NotSolid("a pattern")),
    }
}

fn label(svg: &str) -> Option<String> {
    let start = svg.find("<title")?;
    let open = start + svg[start..].find('>')? + 1;
    let close = open + svg[open..].find("</title>")?;
    let text = svg[open..close].split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'");
    (!text.is_empty()).then_some(text)
}

fn number(out: &mut String, v: f32) {
    let v = (v * 1000.0).round() / 1000.0;
    let _ = write!(out, "{}", if v == 0.0 { 0.0 } else { v });
}

fn path_data(path: &tiny_skia_path::Path) -> String {
    let mut out = String::new();
    let points = |out: &mut String, letter: char, ps: &[tiny_skia_path::Point]| {
        out.push(letter);
        for (i, p) in ps.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            number(out, p.x);
            out.push(' ');
            number(out, p.y);
        }
    };
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => points(&mut out, 'M', &[p]),
            PathSegment::LineTo(p) => points(&mut out, 'L', &[p]),
            PathSegment::QuadTo(a, p) => points(&mut out, 'Q', &[a, p]),
            PathSegment::CubicTo(a, b, p) => points(&mut out, 'C', &[a, b, p]),
            PathSegment::Close => out.push('Z'),
        }
    }
    out
}

fn layers(group: &usvg::Group, out: &mut Vec<Layer>) -> Result<(), Refusal> {
    if group.opacity().get() < 1.0 {
        return Err(Refusal::NotSolid("transparency"));
    }
    if group.clip_path().is_some() || group.mask().is_some() {
        return Err(Refusal::NotSolid("a clip or a mask"));
    }
    if !group.filters().is_empty() {
        return Err(Refusal::NotSolid("a filter"));
    }
    for node in group.children() {
        match node {
            Node::Group(g) => layers(g, out)?,
            Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let ts = p.abs_transform();
                let mut drawn = Vec::new();
                if let Some(fill) = p.fill() {
                    let ink = paint(fill.paint(), fill.opacity().get())?;
                    if let Some(data) = p.data().clone().transform(ts) {
                        drawn.push(Layer { ink, path: path_data(&data), even_odd: fill.rule() == FillRule::EvenOdd });
                    }
                }
                if let Some(stroke) = p.stroke() {
                    let ink = paint(stroke.paint(), stroke.opacity().get())?;
                    let dashed = match stroke.dasharray() {
                        Some(dashes) => StrokeDash::new(dashes.to_vec(), stroke.dashoffset()).and_then(|d| p.data().dash(&d, 8.0)),
                        None => Some(p.data().clone()),
                    };
                    if let Some(outline) = dashed.and_then(|d| d.stroke(&stroke.to_tiny_skia(), 8.0)).and_then(|o| o.transform(ts)) {
                        drawn.push(Layer { ink, path: path_data(&outline), even_odd: false });
                    }
                }
                if p.paint_order() == usvg::PaintOrder::StrokeAndFill {
                    drawn.reverse();
                }
                out.extend(drawn);
            }
            Node::Image(_) => return Err(Refusal::NotSolid("an embedded image")),
            Node::Text(_) => return Err(Refusal::NotSolid("text, which must be turned into outlines first")),
        }
    }
    Ok(())
}

/// Compiles one SVG document into a picture called `name`, or says why it can't be one.
pub fn compile(name: &str, svg: &str) -> Result<Picture, Refusal> {
    let label = label(svg).ok_or(Refusal::NoLabel)?;
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| Refusal::Unreadable(e.to_string()))?;
    let mut out = Vec::new();
    layers(tree.root(), &mut out)?;
    let size = tree.size();
    Ok(Picture { name: name.to_string(), label, width: size.width(), height: size.height(), layers: out })
}

/// The name of the `static` a picture compiles to: its file name in capitals, with anything else as underscores.
pub fn ident(name: &str) -> String {
    let ident: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    if ident.starts_with(|c: char| c.is_ascii_digit()) { format!("_{ident}") } else { ident }
}

/// The Rust for one picture, a `static` of `{prefix}::Picture`.
pub fn code(p: &Picture, prefix: &str) -> String {
    let mut layers = String::new();
    for l in &p.layers {
        let _ = write!(layers, "{prefix}::Layer {{ ink: {prefix}::Ink::{}, path: {:?}, even_odd: {} }}, ", l.ink, l.path, l.even_odd);
    }
    format!(
        "/// {}\npub static {}: {prefix}::Picture = {prefix}::Picture {{ label: {:?}, width: {:?}, height: {:?}, layers: &[{layers}] }};\n",
        p.label,
        ident(&p.name),
        p.label,
        p.width,
        p.height
    )
}

/// The Rust for `ALL`, every picture by name, in name order.
pub fn all(names: &[String], prefix: &str) -> String {
    let entries: Vec<String> = names.iter().map(|n| format!("({n:?}, &{})", ident(n))).collect();
    format!("/// Every picture, by the name of its file, in name order.\npub static ALL: [(&str, &{prefix}::Picture); {}] = [{}];\n", names.len(), entries.join(", "))
}

/// Every SVG file in a folder, in name order, compiled or refused.
pub fn read(dir: &Path) -> Result<Vec<(String, Compiled)>, String> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let shown = file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let name = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let picture = std::fs::read_to_string(&file).map_err(|e| Refusal::Unreadable(e.to_string())).and_then(|svg| compile(&name, &svg));
        out.push((shown, picture));
    }
    Ok(out)
}

/// Compiles every SVG file in `dir` into `out`, for a build script; the build fails, naming each file and why, if any is refused.
pub fn engrave(dir: &Path, out: &Path, prefix: &str) -> Result<(), String> {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut code = String::new();
    let mut refused = Vec::new();
    let mut names = Vec::new();
    for (file, picture) in read(dir)? {
        match picture {
            Ok(p) => {
                code.push_str(&self::code(&p, prefix));
                names.push(p.name);
            }
            Err(why) => refused.push(format!("{}: {why}", dir.join(file).display())),
        }
    }
    if !refused.is_empty() {
        return Err(refused.join("\n"));
    }
    code.push_str(&all(&names, prefix));
    std::fs::write(out, code).map_err(|e| format!("{}: {e}", out.display()))
}
