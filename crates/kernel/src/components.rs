use crate::grid::Area;
use crate::ink::Color;
use crate::layout::{CHECK, MARGIN};
use crate::paint::Painter;
use crate::px::Rect;
use crate::text::{Block, Paint, SpanStyle};
use crate::tree::{Kind, Node, Tone, Tree};

/// How the pointer and the keyboard are touching a node right now. Changing it repaints the node without building the tree again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Look {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub caret: u32,
    pub anchor: u32,
}

/// The two lists a frame is drawn from: what lies under, then what lies on top. Boxes on the grid never overlap, so this is the only ordering painting needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Under,
    Over,
}

pub fn shadow(p: &Painter) -> f32 {
    (p.m.cell_h * 0.12).round().max(p.m.hair() * 2.0)
}

pub fn border(p: &mut Painter, r: Rect, t: f32, color: Color) {
    p.fill(Rect::new(r.x, r.y, r.w, t), color);
    p.fill(Rect::new(r.x, r.bottom() - t, r.w, t), color);
    p.fill(Rect::new(r.x, r.y, t, r.h), color);
    p.fill(Rect::new(r.right() - t, r.y, t, r.h), color);
}

/// Proportional or mono text wrapped inside `a`, one line per row.
pub fn wrapped(p: &mut Painter, a: Area, s: &str, mono: bool, weight: f32, color: Color) {
    if s.is_empty() || a.is_empty() {
        return;
    }
    let r = p.rect(a);
    let style = if mono { p.m.mono() } else { p.m.sans(weight) };
    if a.rows <= 1 || mono {
        p.text(s, style, (r.x, r.y), Some(r.w), color);
        return;
    }
    let span = SpanStyle { weight: style.weight as u16, ..SpanStyle::plain(style.face) };
    let clip = p.clip_to(r);
    let line = p.m.cell_h;
    p.text.draw_spans(p.quads, Block { spans: &[(s, span)], size: style.size, line, wrap: Some(r.w) }, Paint { origin: (r.x, r.y), limit: None, colors: &[color], clip: p.clip() });
    p.set_clip(clip);
}

fn tone(p: &Painter, t: Tone) -> Color {
    match t {
        Tone::Ink => p.theme.ink,
        Tone::Muted => p.theme.muted,
        Tone::Faint => p.theme.faint,
        Tone::Danger => p.theme.danger,
        Tone::Ok => p.theme.ok,
        Tone::Warn => p.theme.warn,
    }
}

pub fn checkbox(p: &mut Painter, x: f32, y: f32, on: bool, enabled: bool) {
    let t = p.m.hair();
    let side = (p.m.cell_h * 0.55).round();
    let bx = (x + (p.m.cell_w * CHECK as f32 - side) / 2.0).floor();
    let by = (y + (p.m.cell_h - side) / 2.0).floor();
    let ink = if enabled { p.theme.ink } else { p.theme.faint };
    let frame = if on {
        ink
    } else if enabled {
        p.theme.muted
    } else {
        p.theme.faint
    };
    border(p, Rect::new(bx, by, side, side), t, frame);
    if on {
        let inset = (side * 0.25).round();
        p.fill(Rect::new(bx + inset, by + inset, side - inset * 2.0, side - inset * 2.0), ink);
    }
}

fn button(p: &mut Painter, n: &Node, label: &str, look: Look) {
    let outer = p.rect(n.area);
    let d = shadow(p);
    let body = Rect::new(outer.x, outer.y, outer.w - d, outer.h - d);
    let pressed = n.enabled && look.pressed;
    let r = if pressed { Rect::new(body.x + d, body.y + d, body.w, body.h) } else { body };
    let theme = p.theme;
    let (edge, fill, ink) = if n.enabled { (theme.ink, if look.hovered { theme.hover } else { theme.bg }, theme.ink) } else { (theme.rule, theme.bg, theme.faint) };
    if n.enabled && !pressed {
        p.fill(Rect::new(body.x + d, body.y + d, body.w, body.h), edge);
    }
    p.fill(r, fill);
    let t = p.m.hair();
    border(p, r, t, edge);
    if look.focused {
        border(p, Rect::new(r.x + t * 2.0, r.y + t * 2.0, r.w - t * 4.0, r.h - t * 4.0), t, theme.cursor);
    }
    let mono = p.m.mono();
    let w = p.measure(label, mono);
    let room = r.w - p.m.cell_w * 2.0;
    let x = if w > room { r.x + p.m.cell_w } else { (r.x + (r.w - w) / 2.0).round() };
    let y = (r.y + (r.h - p.m.cell_h) / 2.0).floor();
    p.text(label, mono, (x, y), Some(room.max(0.0)), ink);
}

fn keycap(p: &mut Painter, at: (f32, f32), keys: &str, enabled: bool) -> f32 {
    let mono = p.m.mono();
    let w = p.measure(keys, mono);
    let i = (p.m.cell_h * 0.15).round();
    let cap = Rect::new(at.0, at.1 + i, (w + p.m.cell_w * 0.5).ceil(), p.m.cell_h - i * 2.0);
    let theme = p.theme;
    p.fill(cap, theme.field);
    let ink = if enabled { theme.ink } else { theme.faint };
    p.text(keys, mono, ((at.0 + p.m.cell_w * 0.25).round(), at.1), None, ink);
    cap.w
}

fn action(p: &mut Painter, n: &Node, label: &str, keys: &str, look: Look) {
    let r = p.rect(n.area);
    let theme = p.theme;
    if n.enabled && look.hovered {
        p.fill(r, theme.hover);
    }
    let t = p.m.hair();
    let i = (p.m.cell_h * 0.1).round();
    let chip = Rect::new(r.x, r.y + i, r.w, r.h - i * 2.0);
    border(p, chip, t, if n.enabled { theme.muted } else { theme.rule });
    let mut x = r.x + p.m.cell_w;
    if !keys.is_empty() {
        let mono = p.m.mono();
        let w = p.measure(keys, mono);
        keycap(p, (x - p.m.cell_w * 0.25, r.y), keys, n.enabled);
        x += (w / p.m.cell_w).ceil() * p.m.cell_w + p.m.cell_w;
    }
    let style = p.m.sans(400.0);
    let ink = if !n.enabled {
        theme.faint
    } else if look.pressed {
        theme.muted
    } else {
        theme.ink
    };
    p.text(label, style, (x, r.y), Some(r.right() - x), ink);
}

fn choice(p: &mut Painter, n: &Node, chosen: bool, label: &str, keys: &str, reason: &str, look: Look) {
    let r = p.rect(n.area);
    let theme = p.theme;
    if chosen {
        p.fill(r, theme.selection);
    } else if look.hovered {
        p.fill(r, theme.hover);
    }
    let style = p.m.sans(400.0);
    let ink = if n.enabled { theme.ink } else { theme.faint };
    let mut x = r.x + p.m.cell_w;
    let mono = p.m.mono();
    let keys_w = if keys.is_empty() { 0.0 } else { (p.measure(keys, mono) / p.m.cell_w).ceil() * p.m.cell_w + p.m.cell_w };
    let w = p.text(label, style, (x, r.y), Some(r.w - keys_w - p.m.cell_w * 2.0), ink).width;
    x += (w / p.m.cell_w).ceil() * p.m.cell_w + p.m.cell_w;
    if !reason.is_empty() && x < r.right() - keys_w {
        p.text(reason, style, (x, r.y), Some(r.right() - keys_w - x), theme.muted);
    }
    if !keys.is_empty() {
        keycap(p, (r.right() - keys_w, r.y), keys, n.enabled);
    }
}

fn link(p: &mut Painter, n: &Node, label: &str, look: Look, ink: Color) {
    let r = p.rect(n.area);
    if n.enabled && look.hovered {
        let hover = p.theme.hover;
        p.fill(r, hover);
    }
    let style = p.m.sans(400.0);
    let ink = if !n.enabled {
        p.theme.faint
    } else if look.pressed {
        p.theme.visited
    } else {
        ink
    };
    let w = p.text(label, style, (r.x, r.y), Some(r.w), ink).width.min(r.w);
    if n.enabled {
        let t = if look.hovered || look.focused { p.m.hair() * 2.0 } else { p.m.hair() };
        let y = (r.y + p.m.cell_h * 0.82).round();
        p.fill(Rect::new(r.x, y, w, t), ink);
    }
}

fn field(p: &mut Painter, n: &Node, value: &str, placeholder: &str, look: Look) {
    let r = p.rect(n.area);
    let theme = p.theme;
    p.fill(r, theme.field);
    let clip = p.clip_to(r);
    let mono = p.m.mono();
    let tx = r.x + p.m.cell_w * 0.5;
    if value.is_empty() {
        p.text(placeholder, mono, (tx, r.y), Some(r.w - p.m.cell_w), theme.faint);
        if look.focused {
            p.fill(Rect::new(tx.floor(), r.y + p.m.cell_h * 0.15, (p.m.scale * 2.0).round().max(1.0), p.m.cell_h * 0.7), theme.ink);
        }
    } else {
        let caret = (look.caret as usize).min(value.len());
        let caret = (0..=caret).rev().find(|&i| value.is_char_boundary(i)).unwrap_or(0);
        let before = p.measure(&value[..caret], mono);
        let room = r.w - p.m.cell_w * 1.5;
        let shift = if look.focused { (before - room).max(0.0) } else { 0.0 };
        let x0 = tx - shift;
        let anchor = (look.anchor as usize).min(value.len());
        let anchor = (0..=anchor).rev().find(|&i| value.is_char_boundary(i)).unwrap_or(0);
        if look.focused && anchor != caret {
            let other = p.measure(&value[..anchor], mono);
            let (a, b) = (before.min(other), before.max(other));
            p.fill(Rect::new((x0 + a).floor(), r.y, (b - a).ceil(), p.m.cell_h), theme.selection);
        }
        p.text(value, mono, (x0, r.y), None, if n.enabled { theme.ink } else { theme.faint });
        if look.focused {
            p.fill(Rect::new((x0 + before).floor(), r.y + p.m.cell_h * 0.15, (p.m.scale * 2.0).round().max(1.0), p.m.cell_h * 0.7), theme.ink);
        }
    }
    p.set_clip(clip);
    let t = p.m.hair();
    let line = if look.focused {
        theme.ink
    } else if look.hovered {
        theme.muted
    } else {
        theme.rule
    };
    p.fill(Rect::new(r.x, r.bottom() - t, r.w, t), line);
}

fn scrollbar(p: &mut Painter, track: Rect, first: u32, shown: u32, count: u32) {
    if count <= shown || track.h <= 0.0 {
        return;
    }
    let t = p.m.hair();
    let mid = (track.x + track.w / 2.0).floor();
    let rule = p.theme.rule;
    p.fill(Rect::new(mid, track.y, t, track.h), rule);
    let thumb_h = (shown as f32 / count as f32 * track.h).max(p.m.cell_h).min(track.h);
    let max = (count - shown).max(1) as f32;
    let y = (track.y + (first as f32 / max).clamp(0.0, 1.0) * (track.h - thumb_h)).round();
    let w = t * 3.0;
    let muted = p.theme.muted;
    p.fill(Rect::new((track.x + (track.w - w) / 2.0).round(), y, w, thumb_h), muted);
}

/// Paints one node's own drawing, not its children's, into one layer.
pub fn paint<C>(p: &mut Painter, tree: &Tree<C>, i: u32, look: Look, layer: Layer) {
    let n = tree.nodes[i as usize];
    if n.area.is_empty() {
        return;
    }
    let text = tree.str(n.text);
    match (layer, n.kind) {
        (Layer::Over, Kind::Page) => {
            let a = Area::new(n.area.col + MARGIN, n.area.row + 1, n.area.cols - MARGIN * 2, 1);
            let style = p.m.sans(600.0);
            let ink = p.theme.ink;
            p.write(a, text, style, ink);
        }
        (Layer::Over, Kind::Text { mono, tone: t, strong }) => {
            let color = tone(p, t);
            wrapped(p, n.area, text, mono, if strong { 600.0 } else { 400.0 }, color);
        }
        (Layer::Over, Kind::Button) => button(p, &n, text, look),
        (Layer::Over, Kind::Action) => action(p, &n, text, tree.str(n.keys), look),
        (Layer::Over, Kind::Choice { chosen }) => choice(p, &n, chosen, text, tree.str(n.keys), tree.str(n.reason), look),
        (Layer::Under, Kind::Palette) => {
            let r = p.rect(n.area);
            let t = p.m.hair();
            let rule = p.theme.rule;
            let width = p.m.width;
            p.fill(Rect::new(0.0, r.y, width, t), rule);
        }
        (Layer::Over, Kind::Link) => {
            let ink = p.theme.link;
            link(p, &n, text, look, ink);
        }
        (Layer::Under, Kind::Check { .. }) if n.enabled && look.hovered => {
            let hover = p.theme.hover;
            p.fill_area(n.area, hover);
        }
        (Layer::Over, Kind::Check { on }) => {
            let r = p.rect(n.area);
            checkbox(p, r.x, r.y, on, n.enabled);
            let ink = if n.enabled { p.theme.ink } else { p.theme.faint };
            let a = Area::new(n.area.col + CHECK + 1, n.area.row, n.area.cols - CHECK - 1, n.area.rows);
            wrapped(p, a, text, false, 400.0, ink);
            if look.focused {
                let t = p.m.hair();
                let cursor = p.theme.cursor;
                border(p, r, t, cursor);
            }
        }
        (Layer::Over, Kind::Field) => field(p, &n, text, tree.str(n.name), look),
        (Layer::Under, Kind::ListRow { selected, focused, .. }) => {
            let theme = p.theme;
            if selected {
                p.fill_area(n.area, theme.selection);
            } else if look.hovered {
                p.fill_area(n.area, theme.hover);
            }
            if focused {
                let r = p.rect(n.area);
                let t = p.m.hair();
                p.fill(Rect::new(r.x, r.y, t * 2.0, r.h), theme.ink);
            }
        }
        (Layer::Over, Kind::List { count, first, shown }) => {
            let a = n.area;
            let track = p.rect(Area::new(a.right() - 1, a.row, 1, a.rows));
            scrollbar(p, track, first, shown, count);
        }
        (Layer::Over, Kind::Empty) => {}
        (Layer::Over, Kind::Status) => {
            let muted = p.theme.muted;
            let style = p.m.sans(400.0);
            p.write(n.area, text, style, muted);
        }
        (Layer::Under, Kind::Note) => {
            let r = p.rect(n.area);
            let t = p.m.hair();
            let rule = p.theme.rule;
            let width = p.m.width;
            p.fill(Rect::new(0.0, r.y, width, t), rule);
        }
        (Layer::Over, Kind::Note) => {
            let color = if n.enabled { p.theme.ink } else { p.theme.danger };
            let style = p.m.sans(400.0);
            p.write(n.area, text, style, color);
        }
        _ => {}
    }
}
