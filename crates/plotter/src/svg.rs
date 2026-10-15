use std::fmt::Write;

use ledger::{Field, Value};

use crate::board::{Board, End, Ink, Kind};
use crate::geometry::Bounds;

/// The board as an SVG picture, every shape in its stacking order, in the light theme's inks, carrying the board itself in its metadata so [`Board::from_svg`] opens it again. An empty board is an empty picture.
pub fn svg(board: &Board) -> String {
    let stacked = board.stacked();
    let area = stacked.iter().map(|s| s.bounds()).reduce(|a, b| a.union(&b)).unwrap_or_default().expand(16.0);
    let mut out = String::new();
    let _ = writeln!(out, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="{}">"#, num(area.x), num(area.y), num(area.w), num(area.h), num(area.w), num(area.h));
    let _ = writeln!(
        out,
        r#"<defs><marker id="head" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="context-stroke"/></marker></defs>"#
    );
    let _ = writeln!(out, "<metadata>{}{}</metadata>", MARK, base64(&board.value().to_bytes()));
    for shape in stacked {
        let color = ink(shape.ink);
        let b = Bounds::new(shape.x, shape.y, shape.w, shape.h);
        let stroke = format!(r#"fill="none" stroke="{color}" stroke-width="2""#);
        let _ = match &shape.kind {
            Kind::Rect => writeln!(out, r#"<rect x="{}" y="{}" width="{}" height="{}" {stroke}/>"#, num(b.x), num(b.y), num(b.w), num(b.h)),
            Kind::Ellipse => writeln!(out, r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {stroke}/>"#, num(b.center().x), num(b.center().y), num(b.w / 2.0), num(b.h / 2.0)),
            Kind::Text(text) => {
                writeln!(out, r#"<text x="{}" y="{}" fill="{color}" font-family="IBM Plex Sans, sans-serif" font-size="16" dominant-baseline="hanging">{}</text>"#, num(b.x), num(b.y), escape(text))
            }
            Kind::Draw(_) => {
                let path: Vec<String> = shape.stroke().unwrap_or_default().iter().map(|p| format!("{},{}", num(p.x), num(p.y))).collect();
                writeln!(out, r#"<polygon points="{}" fill="{color}"/>"#, path.join(" "))
            }
            Kind::Arrow => {
                let (a, z) = (shape.end(End::Start), shape.end(End::End));
                writeln!(out, r#"<line x1="{}" y1="{}" x2="{}" y2="{}" {stroke} marker-end="url(#head)"/>"#, num(a.x), num(a.y), num(z.x), num(z.y))
            }
        };
    }
    out.push_str("</svg>\n");
    out
}

const MARK: &str = "graticule-board:";

impl Board {
    /// The board an SVG picture made by [`svg`] carries, or `None` for any other picture.
    pub fn from_svg(svg: &str) -> Option<Board> {
        let start = svg.find(MARK)? + MARK.len();
        let end = start + svg[start..].find('<')?;
        let value = Value::from_bytes(&unbase64(svg[start..end].trim())?).ok()?;
        Board::from_value(&value)
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn unbase64(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u32> = text.bytes().filter(|&c| c != b'=').map(|c| ALPHABET.iter().position(|&a| a == c).map(|p| p as u32)).collect::<Option<_>>()?;
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &d)| n | d << (18 - 6 * i));
        for i in 0..chunk.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

fn ink(ink: Ink) -> &'static str {
    match ink {
        Ink::Ink => "#1c1b1a",
        Ink::Muted => "#6b6862",
        Ink::Link => "#2f5bd3",
        Ink::Ok => "#2e7d4f",
        Ink::Warn => "#a86400",
        Ink::Danger => "#c2352b",
    }
}

fn num(v: f64) -> String {
    let rounded = (v * 100.0).round() / 100.0;
    if rounded == rounded.trunc() { format!("{}", rounded as i64) } else { format!("{rounded}") }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
