//! The taste laws that judge one laid-out tree, shared by Assay, which reports them, and the runner, which answers Scope and marks them on the window.

use crate::grid::Area;
use crate::tree::{NONE, Tree};

/// A box that breaks a taste law, and what is wrong, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flaw {
    pub node: u32,
    pub said: String,
}

/// Every box that leaves the window or the box it sits in.
pub fn off_grid<C>(tree: &Tree<C>, window: Area) -> Vec<Flaw> {
    let within = |inner: Area, outer: Area| inner.col >= outer.col && inner.row >= outer.row && inner.right() <= outer.right() && inner.bottom() <= outer.bottom();
    let mut flaws = Vec::new();
    for (i, n) in tree.nodes.iter().enumerate() {
        if n.area.is_empty() {
            continue;
        }
        let said = if !within(n.area, window) {
            format!("a {} {} leaves the window, {} by {} cells", n.kind.role(), at_cells(n.area), window.cols, window.rows)
        } else if n.parent != NONE && !within(n.area, tree.nodes[n.parent as usize].area) {
            let parent = tree.nodes[n.parent as usize];
            format!("a {} {} leaves its parent, a {} {}", n.kind.role(), at_cells(n.area), parent.kind.role(), at_cells(parent.area))
        } else {
            continue;
        };
        flaws.push(Flaw { node: i as u32, said });
    }
    flaws
}

fn at_cells(a: Area) -> String {
    format!("at column {}, row {}, {} by {} cells", a.col, a.row, a.cols, a.rows)
}
