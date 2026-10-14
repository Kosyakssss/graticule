use crate::px::Rect;

pub fn drawn(c: char) -> bool {
    matches!(c as u32, 0x2500..=0x259F | 0x2800..=0x28FF) || matches!(c, '\u{2318}' | '\u{21E7}' | '\u{2325}' | '\u{2303}' | '\u{25B4}' | '\u{25B8}' | '\u{25BE}' | '\u{25C2}')
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum W {
    None,
    Light,
    Heavy,
    Double,
}

const ARMS: &str = concat!(
    "0101", "0202", "1010", "2020", "....", "....", "....", "....", "....", "....", "....", "....", "0110", "0210", "0120", "0220", "0011", "0012", "0021", "0022", "1100", "1200", "2100", "2200",
    "1001", "1002", "2001", "2002", "1110", "1210", "2110", "1120", "2120", "2210", "1220", "2220", "1011", "1012", "2011", "1021", "2021", "2012", "1022", "2022", "0111", "0112", "0211", "0212",
    "0121", "0122", "0221", "0222", "1101", "1102", "1201", "1202", "2101", "2102", "2201", "2202", "1111", "1112", "1211", "1212", "2111", "1121", "2121", "2112", "2211", "1122", "1221", "2212",
    "1222", "2122", "2221", "2222", "....", "....", "....", "....", "0303", "3030", "0310", "0130", "0330", "0013", "0031", "0033", "1300", "3100", "3300", "1003", "3001", "3003", "1310", "3130",
    "3330", "1013", "3031", "3033", "0313", "0131", "0333", "1303", "3101", "3303", "1313", "3131", "3333", "....", "....", "....", "....", "....", "....", "....", "0001", "1000", "0100", "0010",
    "0002", "2000", "0200", "0020", "0201", "1020", "0102", "2010",
);

fn arms(c: char) -> Option<[W; 4]> {
    let i = (c as u32).checked_sub(0x2500)? as usize;
    let code = ARMS.get(i * 4..i * 4 + 4)?;
    if code.starts_with('.') {
        return None;
    }
    let mut out = [W::None; 4];
    for (o, b) in out.iter_mut().zip(code.bytes()) {
        *o = match b {
            b'1' => W::Light,
            b'2' => W::Heavy,
            b'3' => W::Double,
            _ => W::None,
        };
    }
    Some(out)
}

pub struct Pen {
    pub t: f32,
    pub rects: Vec<Rect>,
    pub alpha: f32,
}

struct Strokes([(f32, f32); 2], usize);

impl Strokes {
    fn all(&self) -> &[(f32, f32)] {
        &self.0[..self.1]
    }
}

fn lines(w: W, t: f32) -> Strokes {
    let span = |a: f32, b: f32| ((a * t).floor(), (b * t).floor().max((a * t).floor() + 1.0));
    match w {
        W::None => Strokes([(0.0, 0.0); 2], 0),
        W::Light => Strokes([span(0.0, 1.0); 2], 1),
        W::Heavy => Strokes([span(-0.5, 1.5); 2], 1),
        W::Double => Strokes([span(-2.0, -1.0), span(1.0, 2.0)], 2),
    }
}

fn lo(w: W, t: f32) -> Option<f32> {
    lines(w, t).all().first().map(|l| l.0)
}

fn hi(w: W, t: f32) -> Option<f32> {
    lines(w, t).all().last().map(|l| l.1)
}

fn inner(line: (f32, f32), own: W, neg: W, pos: W, t: f32, forward: bool) -> f32 {
    let edge = |w| if forward { lo(w, t) } else { hi(w, t) };
    let gap = if forward { t } else { -t };
    let any_double = neg == W::Double || pos == W::Double;
    if own != W::Double {
        if any_double {
            return gap;
        }
        let edges = [neg, pos].into_iter().filter_map(edge);
        let covering = if forward { edges.fold(f32::MAX, f32::min) } else { edges.fold(f32::MIN, f32::max) };
        return if covering.abs() < f32::MAX {
            covering
        } else if forward {
            line.0
        } else {
            line.1
        };
    }
    let (near, far) = if line.1 <= 0.0 { (neg, pos) } else { (pos, neg) };
    match (near, far) {
        (W::Double, _) => gap,
        (W::None, W::None) => 0.0,
        (W::None, f) => edge(f).unwrap_or(0.0),
        (n, _) => edge(n).unwrap_or(0.0),
    }
}

fn box_lines(c: char, r: Rect, pen: &mut Pen) -> bool {
    let Some([up, right, down, left]) = arms(c) else { return false };
    let t = pen.t;
    let cx = (r.x + (r.w / 2.0).floor()).floor();
    let cy = (r.y + (r.h / 2.0).floor()).floor();
    for (w, horizontal, forward) in [(right, true, true), (left, true, false), (down, false, true), (up, false, false)] {
        let (neg, pos) = if horizontal { (up, down) } else { (left, right) };
        for &line in lines(w, t).all() {
            let at = inner(line, w, neg, pos, t, forward);
            let rect = match (horizontal, forward) {
                (true, true) => Rect::new(cx + at, cy + line.0, r.right() - cx - at, line.1 - line.0),
                (true, false) => Rect::new(r.x, cy + line.0, cx + at - r.x, line.1 - line.0),
                (false, true) => Rect::new(cx + line.0, cy + at, line.1 - line.0, r.bottom() - cy - at),
                (false, false) => Rect::new(cx + line.0, r.y, line.1 - line.0, cy + at - r.y),
            };
            if rect.w > 0.0 && rect.h > 0.0 {
                pen.rects.push(rect);
            }
        }
    }
    true
}

fn dashes(c: char, r: Rect, pen: &mut Pen) -> bool {
    let (n, heavy, vertical) = match c {
        '┄' => (3, false, false),
        '┅' => (3, true, false),
        '┆' => (3, false, true),
        '┇' => (3, true, true),
        '┈' => (4, false, false),
        '┉' => (4, true, false),
        '┊' => (4, false, true),
        '┋' => (4, true, true),
        '╌' => (2, false, false),
        '╍' => (2, true, false),
        '╎' => (2, false, true),
        '╏' => (2, true, true),
        _ => return false,
    };
    let t = pen.t;
    let (a0, a1) = if heavy { ((-0.5 * t).floor(), (1.5 * t).floor()) } else { (0.0, t) };
    let len = if vertical { r.h } else { r.w };
    let seg = len / n as f32;
    let gap = (seg / 3.0).max(t).round();
    for i in 0..n {
        let s = (i as f32 * seg + gap / 2.0).round();
        let e = ((i + 1) as f32 * seg - gap / 2.0).round();
        pen.rects.push(if vertical {
            let cx = (r.x + (r.w / 2.0).floor()).floor();
            Rect::new(cx + a0, r.y + s, a1 - a0, e - s)
        } else {
            let cy = (r.y + (r.h / 2.0).floor()).floor();
            Rect::new(r.x + s, cy + a0, e - s, a1 - a0)
        });
    }
    true
}

fn blocks(c: char, r: Rect, pen: &mut Pen) -> bool {
    let code = c as u32;
    let frac = |n: u32, of: f32| (of * n as f32 / 8.0).round();
    let (hw, hh) = ((r.w / 2.0).round(), (r.h / 2.0).round());
    let quads = |ul: bool, ur: bool, ll: bool, lr: bool, pen: &mut Pen| {
        for (on, x, y, w, h) in [(ul, r.x, r.y, hw, hh), (ur, r.x + hw, r.y, r.w - hw, hh), (ll, r.x, r.y + hh, hw, r.h - hh), (lr, r.x + hw, r.y + hh, r.w - hw, r.h - hh)] {
            if on {
                pen.rects.push(Rect::new(x, y, w, h));
            }
        }
    };
    match code {
        0x2580 => pen.rects.push(Rect::new(r.x, r.y, r.w, hh)),
        0x2581..=0x2588 => {
            let h = frac(code - 0x2580, r.h);
            pen.rects.push(Rect::new(r.x, r.bottom() - h, r.w, h));
        }
        0x2589..=0x258F => {
            let w = frac(0x2590 - code, r.w);
            pen.rects.push(Rect::new(r.x, r.y, w, r.h));
        }
        0x2590 => pen.rects.push(Rect::new(r.x + hw, r.y, r.w - hw, r.h)),
        0x2591..=0x2593 => {
            pen.alpha = (code - 0x2590) as f32 * 0.25;
            pen.rects.push(r);
        }
        0x2594 => pen.rects.push(Rect::new(r.x, r.y, r.w, frac(1, r.h))),
        0x2595 => {
            let w = frac(1, r.w);
            pen.rects.push(Rect::new(r.right() - w, r.y, w, r.h));
        }
        0x2596 => quads(false, false, true, false, pen),
        0x2597 => quads(false, false, false, true, pen),
        0x2598 => quads(true, false, false, false, pen),
        0x2599 => quads(true, false, true, true, pen),
        0x259A => quads(true, false, false, true, pen),
        0x259B => quads(true, true, true, false, pen),
        0x259C => quads(true, true, false, true, pen),
        0x259D => quads(false, true, false, false, pen),
        0x259E => quads(false, true, true, false, pen),
        0x259F => quads(false, true, true, true, pen),
        _ => return false,
    }
    true
}

fn braille(c: char, r: Rect, pen: &mut Pen) -> bool {
    let Some(bits) = (c as u32).checked_sub(0x2800).filter(|b| *b < 256) else { return false };
    const DOTS: [(u32, u32); 8] = [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (0, 3), (1, 3)];
    let (cw, ch) = (r.w / 2.0, r.h / 4.0);
    let d = (cw.min(ch) * 0.6).round().max(1.0);
    for (i, (col, row)) in DOTS.iter().enumerate() {
        if bits & (1 << i) != 0 {
            let x = (r.x + *col as f32 * cw + (cw - d) / 2.0).round();
            let y = (r.y + *row as f32 * ch + (ch - d) / 2.0).round();
            pen.rects.push(Rect::new(x, y, d, d));
        }
    }
    true
}

pub enum Shape {
    Rects,
    Path(String, Option<f32>),
}

pub fn draw(c: char, r: Rect, pen: &mut Pen) -> Option<Shape> {
    pen.alpha = 1.0;
    if box_lines(c, r, pen) || dashes(c, r, pen) || blocks(c, r, pen) || braille(c, r, pen) {
        return Some(Shape::Rects);
    }
    let t = pen.t;
    let (w, h) = (r.w, r.h);
    let cx = (w / 2.0).floor() + t / 2.0;
    let cy = (h / 2.0).floor() + t / 2.0;
    let rad = cx.min(cy).min(w - cx).min(h - cy);
    let path = match c {
        '╭' => format!("M {cx} {h} L {cx} {} Q {cx} {cy} {} {cy} L {w} {cy}", cy + rad, cx + rad),
        '╮' => format!("M {cx} {h} L {cx} {} Q {cx} {cy} {} {cy} L 0 {cy}", cy + rad, cx - rad),
        '╯' => format!("M {cx} 0 L {cx} {} Q {cx} {cy} {} {cy} L 0 {cy}", cy - rad, cx - rad),
        '╰' => format!("M {cx} 0 L {cx} {} Q {cx} {cy} {} {cy} L {w} {cy}", cy - rad, cx + rad),
        '╱' => format!("M {w} 0 L 0 {h}"),
        '╲' => format!("M 0 0 L {w} {h}"),
        '╳' => format!("M {w} 0 L 0 {h} M 0 0 L {w} {h}"),
        '\u{21E7}' => {
            let (l, r, top, mid, bottom) = (w * 0.12, w * 0.88, h * 0.28, h * 0.52, h * 0.74);
            let (sl, sr) = (w * 0.34, w * 0.66);
            format!("M {cx} {top} L {r} {mid} L {sr} {mid} L {sr} {bottom} L {sl} {bottom} L {sl} {mid} L {l} {mid} Z")
        }
        '\u{2318}' => {
            let (half, rr) = (w * 0.12, w * 0.15);
            let k = rr * 0.552_284_8;
            let (x0, x1, y0, y1) = (cx - half, cx + half, cy - half, cy + half);
            let (e, mut d) = (half + rr, String::new());
            d.push_str(&format!("M {x0} {} L {x0} {} M {x1} {} L {x1} {} M {} {y0} L {} {y0} M {} {y1} L {} {y1} ", cy - e, cy + e, cy - e, cy + e, cx - e, cx + e, cx - e, cx + e));
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let (ox, oy) = (cx + sx * e, cy + sy * e);
                d.push_str(&format!(
                    "M {} {oy} C {} {} {} {} {ox} {} C {} {} {} {} {} {oy} C {} {} {} {} {ox} {} C {} {} {} {} {} {oy} ",
                    ox + rr,
                    ox + rr,
                    oy + k,
                    ox + k,
                    oy + rr,
                    oy + rr,
                    ox - k,
                    oy + rr,
                    ox - rr,
                    oy + k,
                    ox - rr,
                    ox - rr,
                    oy - k,
                    ox - k,
                    oy - rr,
                    oy - rr,
                    ox + k,
                    oy - rr,
                    ox + rr,
                    oy - k,
                    ox + rr
                ));
            }
            d
        }
        '\u{2325}' => {
            let (l, r, top, bottom) = (w * 0.1, w * 0.9, h * 0.34, h * 0.68);
            format!("M {l} {top} L {} {top} L {} {bottom} L {r} {bottom} M {} {top} L {r} {top}", w * 0.36, w * 0.64, w * 0.62)
        }
        '\u{25B8}' => return Some(Shape::Path(format!("M {} {} L {} {cy} L {} {} Z", w * 0.25, h * 0.32, w * 0.8, w * 0.25, h * 0.68), None)),
        '\u{25C2}' => return Some(Shape::Path(format!("M {} {} L {} {cy} L {} {} Z", w * 0.75, h * 0.32, w * 0.2, w * 0.75, h * 0.68), None)),
        '\u{25BE}' => return Some(Shape::Path(format!("M {} {} L {} {} L {cx} {} Z", w * 0.15, h * 0.38, w * 0.85, h * 0.38, h * 0.66), None)),
        '\u{25B4}' => return Some(Shape::Path(format!("M {} {} L {} {} L {cx} {} Z", w * 0.15, h * 0.62, w * 0.85, h * 0.62, h * 0.34), None)),
        '\u{2303}' => format!("M {} {} L {cx} {} L {} {}", w * 0.18, h * 0.5, h * 0.3, w * 0.82, h * 0.5),
        _ => return None,
    };
    Some(Shape::Path(path, Some(t)))
}
