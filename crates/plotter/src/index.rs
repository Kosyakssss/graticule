use ledger::Key;

use crate::board::{Board, Kind, Shape};
use crate::geometry::{Bounds, Point};

/// Where every shape on a board is, for finding the ones in a box without looking at the rest: a tree of boxes packed from the bottom up, sixteen to a box, by sort-tile-recursive packing. Build it off the main thread, as a derived value of the board; a query then costs what it finds, so a board of a million shapes is drawn, hit and snapped to at the cost of the shapes near the pointer or on screen.
#[derive(Clone, Debug, Default)]
pub struct Index {
    entries: Vec<Entry>,
    levels: Vec<Vec<Node>>,
    top: Option<String>,
}

/// One shape in an index: its box, its key, where it is in the stack counting from the bottom, and whether it is an arrow.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub bounds: Bounds,
    pub key: Key<Shape>,
    pub rank: u32,
    pub arrow: bool,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    bounds: Bounds,
    start: u32,
    end: u32,
}

const FANOUT: usize = 16;

impl Index {
    /// The index of every shape on `board`.
    pub fn of(board: &Board) -> Index {
        let stacked = board.stacked();
        let top = stacked.last().map(|s| s.index.clone());
        let mut entries: Vec<Entry> = stacked.into_iter().zip(0..).map(|(s, rank)| Entry { bounds: s.bounds(), key: s.id, rank, arrow: s.kind == Kind::Arrow }).collect();
        let mut levels = Vec::new();
        let mut below: Vec<Node> = pack(&mut entries, |e| e.bounds);
        while below.len() > 1 {
            let above = pack(&mut below, |n| n.bounds);
            levels.push(below);
            below = above;
        }
        levels.push(below);
        Index { entries, levels, top }
    }

    /// `shape` placed on top of the stack.
    pub fn above(&self, mut shape: Shape) -> Shape {
        shape.index = crate::order::between(self.top(), None);
        shape
    }

    /// Where the top of the stack is, for putting a new shape above it without looking at every shape.
    pub fn top(&self) -> Option<&str> {
        self.top.as_deref()
    }

    /// How many shapes it holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The box around every shape, or `None` for an empty board.
    pub fn bounds(&self) -> Option<Bounds> {
        self.levels.last().and_then(|top| top.first()).map(|n| n.bounds)
    }

    /// The shapes whose boxes touch `area`, from the bottom of the stack to the top.
    pub fn query(&self, area: Bounds) -> Vec<Entry> {
        let mut found = Vec::new();
        let Some(top) = self.levels.len().checked_sub(1) else { return found };
        let mut pending: Vec<(usize, u32)> = (0..self.levels[top].len() as u32).map(|i| (top, i)).collect();
        while let Some((level, i)) = pending.pop() {
            let node = self.levels[level][i as usize];
            if !node.bounds.intersects(&area) {
                continue;
            }
            match level.checked_sub(1) {
                Some(below) => pending.extend((node.start..node.end).map(|c| (below, c))),
                None => found.extend(self.entries[node.start as usize..node.end as usize].iter().filter(|e| e.bounds.intersects(&area))),
            }
        }
        found.sort_unstable_by_key(|e| e.rank);
        found
    }

    /// The topmost shape at `p` on `board`, within `margin` of a line, among the ones this index knows.
    pub fn hit(&self, board: &Board, p: Point, margin: f64) -> Option<Key<Shape>> {
        let near = Bounds::new(p.x - margin, p.y - margin, margin * 2.0, margin * 2.0);
        self.query(near).into_iter().rev().filter_map(|e| board.shapes.find(e.key)).find(|s| s.geometry().hit(p, margin)).map(|s| s.id)
    }
}

fn pack<T>(items: &mut [T], bounds: impl Fn(&T) -> Bounds) -> Vec<Node> {
    let groups = items.len().div_ceil(FANOUT);
    let slices = (groups as f64).sqrt().ceil().max(1.0) as usize;
    let across = |t: &T| bounds(t).center().x;
    let down = |t: &T| bounds(t).center().y;
    items.sort_unstable_by(|a, b| across(a).total_cmp(&across(b)));
    let mut nodes = Vec::with_capacity(groups);
    let mut start = 0;
    for slice in items.chunks_mut(slices * FANOUT) {
        slice.sort_unstable_by(|a, b| down(a).total_cmp(&down(b)));
        for group in slice.chunks(FANOUT) {
            let around = group.iter().map(&bounds).reduce(|a, b| a.union(&b)).unwrap_or_default();
            nodes.push(Node { bounds: around, start: start as u32, end: (start + group.len()) as u32 });
            start += group.len();
        }
    }
    nodes
}
