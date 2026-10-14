use std::hash::{Hash, Hasher};

use crate::grid::Area;
use crate::ink::{Color, Ink};
use crate::layout::{CHECK, MARGIN};
use crate::look::CheckMark;
use crate::paint::Painter;
use crate::picture::Picture;
use crate::px::Rect;
use crate::text::{Block, Data, Outline, Paint, SpanStyle};
use crate::tree::{Kind, Node, Tone, Tree};
use crate::value::Value;

/// How the pointer and the keyboard are touching a node right now. Changing it repaints the node without building the tree again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Look {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub caret: u32,
    pub anchor: u32,
    /// What an input method is composing in this field, in the tree's text.
    pub composing: crate::tree::Range32,
    /// The bytes of this node's text chosen with the pointer, as a start and an end.
    pub chosen: (u32, u32),
    /// Whether the node changed in a way the user didn't watch happen, such as an undo or something arriving, and is marked until their next input.
    pub marked: bool,
    /// Whether a row being dragged would land at this node's top or bottom edge.
    pub landing: Landing,
}

/// Where a dragged row would land against the row under it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Landing {
    #[default]
    Not,
    Above,
    Below,
}

/// The two lists a frame is drawn from: what lies under, then what lies on top. Boxes on the grid never overlap, so this is the only ordering painting needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Under,
    Over,
}

pub fn shadow(p: &Painter) -> f32 {
    p.style.shadow(p.m)
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
    let t = p.style.line(p.m);
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
    match (on, p.style.check) {
        (false, _) => {}
        (true, CheckMark::Box) => {
            let inset = (side * 0.25).round();
            p.fill(Rect::new(bx + inset, by + inset, side - inset * 2.0, side - inset * 2.0), ink);
        }
        (true, CheckMark::Tick) => draw_picture(p, Rect::new(bx, by, side, side), &crate::picture::icons::CHECK, Some(ink)),
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
    let t = p.style.line(p.m);
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
    let t = p.style.line(p.m);
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

fn entry(p: &mut Painter, n: &Node, label: &str, keys: &str) {
    let r = p.rect(n.area);
    let theme = p.theme;
    let mut right = r.right();
    if !keys.is_empty() {
        let w = p.measure(keys, p.m.mono());
        right -= (w / p.m.cell_w).ceil() * p.m.cell_w + p.m.cell_w * 0.5;
        keycap(p, (right, r.y), keys, n.enabled);
        right -= p.m.cell_w;
    }
    let ink = if n.enabled { theme.ink } else { theme.faint };
    let style = p.m.sans(400.0);
    p.text(label, style, (r.x, r.y), Some((right - r.x).max(0.0)), ink);
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
        let t = if look.hovered || look.focused { p.style.line(p.m) * 2.0 } else { p.style.line(p.m) };
        let y = (r.y + p.m.cell_h * 0.82).round();
        p.fill(Rect::new(r.x, y, w, t), ink);
    }
}

fn field(p: &mut Painter, n: &Node, value: &str, placeholder: &str, composing: &str, look: Look) {
    let r = p.rect(n.area);
    let theme = p.theme;
    p.fill(r, theme.field);
    let clip = p.clip_to(r);
    let mono = p.m.mono();
    let tx = r.x + p.m.cell_w * 0.5;
    let bar = (p.m.scale * 2.0).round().max(1.0);
    if value.is_empty() && composing.is_empty() {
        p.text(placeholder, mono, (tx, r.y), Some(r.w - p.m.cell_w), theme.faint);
        if look.focused {
            p.fill(Rect::new(tx.floor(), r.y + p.m.cell_h * 0.15, bar, p.m.cell_h * 0.7), theme.ink);
        }
    } else {
        let caret = (look.caret as usize).min(value.len());
        let caret = (0..=caret).rev().find(|&i| value.is_char_boundary(i)).unwrap_or(0);
        let before = p.advance(&value[..caret], mono);
        let composed = p.advance(composing, mono);
        let room = r.w - p.m.cell_w * 1.5;
        let shift = if look.focused { (before + composed - room).max(0.0) } else { 0.0 };
        let x0 = tx - shift;
        let anchor = (look.anchor as usize).min(value.len());
        let anchor = (0..=anchor).rev().find(|&i| value.is_char_boundary(i)).unwrap_or(0);
        if look.focused && anchor != caret && composing.is_empty() {
            let other = p.advance(&value[..anchor], mono);
            let (a, b) = (before.min(other), before.max(other));
            p.fill(Rect::new((x0 + a).floor(), r.y, (b - a).ceil(), p.m.cell_h), theme.selection);
        }
        let ink = if n.enabled { theme.ink } else { theme.faint };
        if composing.is_empty() {
            p.text(value, mono, (x0, r.y), None, ink);
        } else {
            p.text(&value[..caret], mono, (x0, r.y), None, ink);
            p.text(composing, mono, (x0 + before, r.y), None, ink);
            p.text(&value[caret..], mono, (x0 + before + composed, r.y), None, ink);
            let t = p.style.line(p.m);
            p.fill(Rect::new((x0 + before).floor(), (r.y + p.m.cell_h * 0.85).round(), composed.ceil(), t), ink);
        }
        if look.focused {
            p.fill(Rect::new((x0 + before + composed).floor(), r.y + p.m.cell_h * 0.15, bar, p.m.cell_h * 0.7), theme.ink);
        }
    }
    p.set_clip(clip);
    let t = p.style.line(p.m);
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
    let t = p.style.line(p.m);
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
            let t = p.style.line(p.m);
            let rule = p.theme.rule;
            let width = p.m.width;
            p.fill(Rect::new(0.0, r.y, width, t), rule);
        }
        (Layer::Under, Kind::Overlay { .. }) => {
            let outer = p.rect(n.area);
            let d = shadow(p);
            let body = Rect::new(outer.x, outer.y, outer.w - d, outer.h - d);
            let (theme, t) = (p.theme, p.style.line(p.m));
            if d > 0.0 {
                p.fill(Rect::new(body.x + d, body.y + d, body.w, body.h), theme.ink);
            }
            p.fill(body, theme.bg);
            border(p, body, t, theme.ink);
        }
        (Layer::Under, Kind::Entry) if n.enabled && (look.hovered || look.focused) => {
            let fill = if look.focused { p.theme.selection } else { p.theme.hover };
            p.fill_area(n.area, fill);
        }
        (Layer::Over, Kind::Entry) => entry(p, &n, text, tree.str(n.keys)),
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
                let t = p.style.line(p.m);
                let cursor = p.theme.cursor;
                border(p, r, t, cursor);
            }
        }
        (Layer::Over, Kind::Field) => field(p, &n, text, tree.str(n.name), tree.str(look.composing), look),
        (Layer::Under, Kind::ListRow { selected, focused, .. }) => {
            let theme = p.theme;
            if selected {
                p.fill_area(n.area, theme.selection);
            } else if look.hovered {
                p.fill_area(n.area, theme.hover);
            }
            if focused {
                let r = p.rect(n.area);
                let t = p.style.line(p.m);
                p.fill(Rect::new(r.x, r.y, t * 2.0, r.h), theme.ink);
            }
            if look.marked {
                let r = p.rect(n.area);
                let t = p.style.line(p.m);
                p.fill(Rect::new(r.right() - t * 3.0, r.y, t * 3.0, r.h), theme.link);
            }
            if look.landing != Landing::Not {
                let r = p.rect(n.area);
                let t = p.style.line(p.m) * 2.0;
                let y = if look.landing == Landing::Above { r.y } else { r.bottom() - t };
                p.fill(Rect::new(r.x, y, r.w, t), theme.ink);
            }
        }
        (Layer::Over, Kind::List { count, first, shown }) => {
            let a = n.area;
            let track = p.rect(Area::new(a.right() - 1, a.row, 1, a.rows));
            scrollbar(p, track, first, shown, count);
        }
        (Layer::Over, Kind::Empty) => {}
        (Layer::Under, Kind::Text { .. } | Kind::Value { .. }) if look.chosen.0 < look.chosen.1 => chosen(p, &n, text, look.chosen.0 as usize..look.chosen.1 as usize),
        (Layer::Under, Kind::Swatch { ink }) => {
            let r = p.rect(n.area);
            let (color, rule) = (p.theme.get(ink), p.theme.rule);
            p.fill(r, color);
            let t = p.style.line(p.m);
            border(p, r, t, rule);
        }
        (Layer::Over, Kind::Picture { picture, .. }) => draw_picture(p, p.rect(n.area), picture, None),
        (Layer::Over, Kind::Value { value }) => typed(p, &n, value, text),
        (Layer::Over, Kind::Rule) => {
            let r = p.rect(n.area);
            let t = p.style.line(p.m);
            let rule = p.theme.rule;
            p.fill(Rect::new(r.x, r.y + p.m.cell_h, r.w, t), rule);
        }
        (Layer::Over, Kind::Keys) => {
            let r = p.rect(n.area);
            if n.enabled {
                keycap(p, (r.x, r.y), text, true);
            } else {
                let (style, muted) = (p.m.sans(400.0), p.theme.muted);
                p.text(text, style, (r.x, r.y), None, muted);
            }
        }
        (Layer::Over, Kind::Gauge { filled, meter }) => gauge(p, &n, text, filled, meter),
        (Layer::Over, Kind::Placeholder { rows }) => {
            let r = p.rect(n.area);
            let field = p.theme.field;
            let inset = (p.m.cell_h * 0.25).round();
            for k in 0..rows.min(n.area.rows) {
                let share = [1.0, 0.85, 0.6][k as usize % 3];
                let w = ((r.w * share) / p.m.cell_w).floor() * p.m.cell_w;
                p.fill(Rect::new(r.x, r.y + k as f32 * p.m.cell_h + inset, w, p.m.cell_h - inset * 2.0), field);
            }
        }
        (Layer::Over, Kind::Tag) => {
            let cell = p.rect(n.area);
            let inset = (p.m.cell_h * 0.1).round();
            let cols = n.area.cols.min(text.chars().count() as i32 + 1);
            let r = Rect::new(cell.x, cell.y + inset, cols as f32 * p.m.cell_w, p.m.cell_h - inset * 2.0);
            let (t, theme) = (p.style.line(p.m), p.theme);
            border(p, r, t, theme.rule);
            let style = p.m.sans(400.0);
            p.text(text, style, ((r.x + p.m.cell_w * 0.5).round(), cell.y), Some(r.w - p.m.cell_w * 0.5), theme.ink);
        }
        (Layer::Over, Kind::Avatar) => {
            let cell = p.rect(n.area);
            let theme = p.theme;
            p.fill(cell, theme.muted);
            let mono = crate::text::TextStyle { weight: 600.0, ..p.m.mono() };
            p.text(text, mono, ((cell.x + p.m.cell_w * 0.5).round(), cell.y), None, theme.bg);
        }
        (Layer::Over, Kind::Callout { ink }) => {
            let r = p.rect(n.area);
            let (t, color) = (p.style.line(p.m), p.theme.get(ink));
            p.fill(Rect::new(r.x, r.y, t * 2.0, r.h), color);
            let icon = match ink {
                Ink::Danger => &crate::picture::icons::ERROR,
                Ink::Warn => &crate::picture::icons::WARNING,
                Ink::Ok => &crate::picture::icons::SUCCESS,
                _ => &crate::picture::icons::INFO,
            };
            draw_picture(p, p.rect(Area::new(n.area.col + 1, n.area.row, 2, 1)), icon, Some(color));
        }
        (Layer::Over, Kind::Card) => {
            let r = p.rect(n.area);
            let (t, theme) = (p.style.line(p.m), p.theme);
            border(p, r, t, theme.rule);
            let style = p.m.sans(600.0);
            p.write(Area::new(n.area.col + 2, n.area.row + 1, n.area.cols - 4, 1), text, style, theme.ink);
        }
        (Layer::Over, Kind::Status) => {
            let muted = p.theme.muted;
            let style = p.m.sans(400.0);
            p.write(n.area, text, style, muted);
        }
        (Layer::Under, Kind::Note) => {
            let r = p.rect(n.area);
            let t = p.style.line(p.m);
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

fn gauge(p: &mut Painter, n: &Node, label: &str, filled: u16, meter: bool) {
    let cols = (n.area.cols - 1 - label.chars().count() as i32).max(1);
    let cell = p.rect(Area::new(n.area.col, n.area.row, cols, 1));
    let (t, theme) = (p.style.line(p.m), p.theme);
    let inset = (p.m.cell_h * 0.3).round();
    let track = Rect::new(cell.x, cell.y + inset, cell.w, p.m.cell_h - inset * 2.0);
    border(p, track, t, theme.rule);
    let w = (track.w * f32::from(filled.min(1000)) / 1000.0).round();
    p.fill(Rect::new(track.x, track.y, w, track.h), if meter { theme.ink } else { theme.link });
    let mono = p.m.mono();
    let x = cell.right() + p.m.cell_w;
    p.text(label, mono, (x, cell.y), None, theme.muted);
}

/// Draws a picture scaled to fit `r` and centred in it, each layer in its ink or all of them in `tint`, from the atlas once it has been drawn at this size.
pub fn draw_picture(p: &mut Painter, r: Rect, picture: &'static Picture, tint: Option<Color>) {
    let scale = (r.w / picture.width).min(r.h / picture.height);
    if scale.is_nan() || scale <= 0.0 {
        return;
    }
    let (w, h) = (picture.width * scale, picture.height * scale);
    let origin = ((r.x + (r.w - w) / 2.0).round(), (r.y + (r.h - h) / 2.0).round());
    let clip = p.clip();
    for (i, layer) in picture.layers.iter().enumerate() {
        let key = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (std::ptr::from_ref(picture) as usize, i, scale.to_bits()).hash(&mut h);
            h.finish()
        };
        let color = tint.unwrap_or_else(|| p.theme.get(layer.ink));
        let outline = Outline { path: Data::Svg(layer.path), stroke: None, even_odd: layer.even_odd, scale };
        p.text.shape(p.quads, key, outline, Paint { origin, limit: None, colors: &[color], clip });
    }
}

fn dotted(p: &mut Painter, r: Rect, t: f32, color: Color) {
    let step = t * 4.0;
    let mut x = r.x;
    while x < r.right() {
        let w = (t * 2.0).min(r.right() - x);
        p.fill(Rect::new(x, r.y, w, t), color);
        p.fill(Rect::new(x, r.bottom() - t, w, t), color);
        x += step;
    }
    let mut y = r.y;
    while y < r.bottom() {
        let h = (t * 2.0).min(r.bottom() - y);
        p.fill(Rect::new(r.x, y, t, h), color);
        p.fill(Rect::new(r.right() - t, y, t, h), color);
        y += step;
    }
}

/// Draws a typed value in its one look: plain for a count and an error, in a box one column wider than its text for the rest.
fn typed(p: &mut Painter, n: &Node, value: Value, text: &str) {
    let theme = p.theme;
    if !value.boxed() {
        let (mono, color) = if value == Value::Count { (true, theme.ink) } else { (false, theme.danger) };
        wrapped(p, n.area, text, mono, 400.0, color);
        return;
    }
    let cell = p.rect(n.area);
    let inset = (p.m.cell_h * 0.1).round();
    let cols = n.area.cols.min(text.chars().count() as i32 + 1);
    let r = Rect::new(cell.x, cell.y + inset, cols as f32 * p.m.cell_w, p.m.cell_h - inset * 2.0);
    let t = p.style.line(p.m);
    let mono = p.m.mono();
    let strong = crate::text::TextStyle { weight: 600.0, ..mono };
    let (style, color) = match value {
        Value::Date => {
            p.fill(r, theme.link);
            (mono, theme.bg)
        }
        Value::State(ink) => {
            p.fill(r, theme.get(ink));
            (strong, theme.bg)
        }
        Value::Version => {
            dotted(p, r, t, theme.muted);
            (mono, theme.ink)
        }
        Value::Product => {
            border(p, r, t * 2.0, theme.ink);
            (strong, theme.ink)
        }
        Value::Code => {
            p.fill(r, theme.field);
            (mono, theme.ink)
        }
        Value::Mode => {
            p.fill(Rect::new(r.x, r.bottom() - t, r.w, t), theme.muted);
            (mono, theme.muted)
        }
        Value::Count | Value::Error => (mono, theme.ink),
    };
    let w = p.measure(text, style);
    let room = r.w - p.m.cell_w;
    let x = if w > room { r.x + p.m.cell_w / 2.0 } else { (r.x + (r.w - w) / 2.0).round() };
    p.text(text, style, (x, cell.y), Some(room.max(0.0)), color);
}

/// The highlight under text chosen with the pointer: the chosen bytes of a text, or the whole of a value.
fn chosen(p: &mut Painter, n: &Node, text: &str, range: std::ops::Range<usize>) {
    let r = p.rect(n.area);
    let selection = p.theme.selection;
    let (style, mono) = match n.kind {
        Kind::Value { .. } => {
            p.fill(r, selection);
            return;
        }
        Kind::Text { mono: true, .. } => (p.m.mono(), true),
        Kind::Text { strong: true, .. } => (p.m.sans(600.0), false),
        _ => (p.m.sans(400.0), false),
    };
    let range = range.start.min(text.len())..range.end.min(text.len());
    if mono || n.area.rows <= 1 {
        let x = p.advance(&text[..range.start], style);
        let w = p.advance(&text[range.clone()], style);
        p.fill(Rect::new((r.x + x).floor(), r.y, w.ceil().min(r.w - x), p.m.cell_h), selection);
        return;
    }
    let spans = [(text, SpanStyle { weight: style.weight as u16, ..SpanStyle::plain(style.face) })];
    let line = p.m.cell_h;
    for [x, y, w, h] in p.text.range_rects(&spans, style.size, line, r.w, range) {
        p.fill(Rect::new((r.x + x).floor(), (r.y + y).floor(), w.ceil(), h.ceil()), selection);
    }
}
