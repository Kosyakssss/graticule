use std::fmt::Write;

use ledger::{Field, Key, Value};

use crate::board::{Board, End, Ink, Kind, Page};
use crate::geometry::Bounds;

/// The board as an SVG picture of `page`, every shape on it in its stacking order, in the light theme's inks, what frames hold clipped to their edges, carrying the board itself in its metadata so [`Board::from_svg`] opens it again. An empty board is an empty picture.
pub fn svg(board: &Board, page: Key<Page>) -> String {
    let stacked = board.on(page);
    let area = stacked.iter().map(|s| s.bounds()).reduce(|a, b| a.union(&b)).unwrap_or_default().expand(16.0);
    let mut out = String::new();
    let _ = writeln!(out, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="{}">"#, num(area.x), num(area.y), num(area.w), num(area.h), num(area.w), num(area.h));
    let _ = writeln!(
        out,
        r#"<defs><marker id="head" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="context-stroke"/></marker></defs>"#
    );
    let _ = writeln!(out, "<metadata>{}{}</metadata>", MARK, crate::base64::encode(&board.value().to_bytes()));
    for (n, shape) in stacked.into_iter().enumerate() {
        let color = ink(shape.ink);
        let b = Bounds::new(shape.x, shape.y, shape.w, shape.h);
        let stroke = format!(r#"fill="none" stroke="{color}" stroke-width="2""#);
        let window = board.window(shape);
        if let Some(w) = window {
            let _ = writeln!(out, r#"<clipPath id="window-{n}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath><g clip-path="url(#window-{n})">"#, num(w.x), num(w.y), num(w.w), num(w.h));
        }
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
            Kind::Frame(name) => {
                let b = shape.bounds();
                let _ = writeln!(out, r#"<rect x="{}" y="{}" width="{}" height="{}" {stroke}/>"#, num(b.x), num(b.y), num(b.w), num(b.h));
                writeln!(out, r#"<text x="{}" y="{}" fill="{color}" font-family="IBM Plex Sans, sans-serif" font-size="12">{}</text>"#, num(b.x), num(b.y - 6.0), escape(name))
            }
            Kind::Image(_) => match board.picture(shape) {
                Some(asset) => writeln!(
                    out,
                    r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" href="data:{};base64,{}"/>"#,
                    num(b.x),
                    num(b.y),
                    num(b.w),
                    num(b.h),
                    asset.media(),
                    asset.data
                ),
                None => Ok(()),
            },
        };
        if window.is_some() {
            out.push_str("</g>\n");
        }
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
        let value = Value::from_bytes(&crate::base64::decode(svg[start..end].trim())?).ok()?;
        Board::from_value(&value)
    }
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
