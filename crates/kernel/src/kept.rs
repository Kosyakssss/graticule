use std::collections::HashMap;
use std::ops::Range;

use crate::components::{self, Layer, Look};
use crate::gpu::Quad;
use crate::grid::Area;
use crate::paint::Painter;
use crate::tree::{Id, Tree};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Slot {
    start: u32,
    len: u32,
    cap: u32,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    slots: [Slot; 2],
    base: usize,
    area: Area,
    hash: u64,
    look: Look,
    frame: u64,
}

/// One layer's quads, kept between frames. Each node owns a range; a node that changed rewrites only its range, and the GPU gets only the bytes that changed.
#[derive(Default)]
pub struct Quads {
    pub quads: Vec<Quad>,
    free: Vec<Slot>,
    pub dirty: Option<Range<usize>>,
}

impl Quads {
    fn touch(&mut self, r: Range<usize>) {
        self.dirty = Some(match self.dirty.take() {
            Some(d) => d.start.min(r.start)..d.end.max(r.end),
            None => r,
        });
    }

    fn write(&mut self, slot: Slot, new: &[Quad]) -> Slot {
        let len = new.len() as u32;
        let mut slot = slot;
        if len > slot.cap {
            self.release(slot);
            slot = self.alloc(len);
        }
        let start = slot.start as usize;
        self.quads[start..start + new.len()].copy_from_slice(new);
        let end = start + slot.len.max(len) as usize;
        for q in &mut self.quads[start + new.len()..end] {
            *q = blank();
        }
        slot.len = len;
        self.touch(start..end.max(start + new.len()));
        slot
    }

    fn alloc(&mut self, len: u32) -> Slot {
        if len == 0 {
            return Slot::default();
        }
        if let Some(i) = self.free.iter().position(|s| s.cap >= len) {
            let s = self.free.swap_remove(i);
            return Slot { start: s.start, len: 0, cap: s.cap };
        }
        let cap = len + len / 2;
        let start = self.quads.len() as u32;
        self.quads.resize(self.quads.len() + cap as usize, blank());
        Slot { start, len: 0, cap }
    }

    fn release(&mut self, slot: Slot) {
        if slot.cap == 0 {
            return;
        }
        let start = slot.start as usize;
        let end = start + slot.len as usize;
        for q in &mut self.quads[start..end] {
            *q = blank();
        }
        if slot.len > 0 {
            self.touch(start..end);
        }
        self.free.push(Slot { len: 0, ..slot });
    }

    fn clear(&mut self) {
        self.quads.clear();
        self.free.clear();
        self.dirty = Some(0..0);
    }
}

fn blank() -> Quad {
    Quad { pos: [0.0; 2], size: [0.0; 2], uv: [0.0; 4], color: [0.0; 4], clip: [0.0; 4], kind: 0, image: 0, pad: [0; 2] }
}

/// The frame as four kept lists of quads, patched from one frame to the next by comparing nodes by id: under and over for the page, then under and over for anchored overlays, which are drawn above everything else.
#[derive(Default)]
pub struct Kept {
    pub layers: [Quads; 4],
    entries: HashMap<Id, Entry>,
    scratch: [Vec<Quad>; 2],
    frame: u64,
    atlas: u64,
    pub repainted: usize,
}

impl Kept {
    /// Forgets everything, so the next frame paints every node.
    pub fn reset(&mut self) {
        self.entries.clear();
        for l in &mut self.layers {
            l.clear();
        }
    }

    /// Brings the kept quads up to date with `tree`, painting only nodes that are new, moved, changed or touched differently than last frame.
    pub fn update<C>(&mut self, p: &mut Painter, tree: &Tree<C>, look: impl Fn(Id) -> Look) {
        if p.text.atlas != self.atlas {
            self.atlas = p.text.atlas;
            self.reset();
        }
        self.frame += 1;
        self.repainted = 0;
        let frame = self.frame;
        for (i, n) in tree.nodes.iter().enumerate() {
            let l = look(n.id);
            let base = if n.over { 2 } else { 0 };
            if let Some(e) = self.entries.get_mut(&n.id)
                && e.base == base
                && e.area == n.area
                && e.hash == n.hash
                && e.look == l
                && e.frame != frame
            {
                e.frame = frame;
                continue;
            }
            self.repainted += 1;
            let old = match self.entries.get(&n.id) {
                Some(e) if e.base == base => e.slots,
                Some(e) => {
                    for (k, slot) in e.slots.into_iter().enumerate() {
                        self.layers[e.base + k].release(slot);
                    }
                    [Slot::default(); 2]
                }
                None => [Slot::default(); 2],
            };
            let mut slots = [Slot::default(); 2];
            for (k, layer) in [Layer::Under, Layer::Over].into_iter().enumerate() {
                let mut out = std::mem::take(&mut self.scratch[k]);
                out.clear();
                let saved = std::mem::replace(p.quads, out);
                components::paint(p, tree, i as u32, l, layer);
                let out = std::mem::replace(p.quads, saved);
                slots[k] = self.layers[base + k].write(old[k], &out);
                self.scratch[k] = out;
            }
            self.entries.insert(n.id, Entry { slots, base, area: n.area, hash: n.hash, look: l, frame });
        }
        let layers = &mut self.layers;
        self.entries.retain(|_, e| {
            if e.frame == frame {
                return true;
            }
            for (k, slot) in e.slots.into_iter().enumerate() {
                layers[e.base + k].release(slot);
            }
            false
        });
    }

    /// Every quad, in drawing order, for drawing in one go.
    pub fn all(&self, out: &mut Vec<Quad>) {
        out.clear();
        for layer in &self.layers {
            out.extend_from_slice(&layer.quads);
        }
    }
}
