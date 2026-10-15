//! The taste laws that judge one laid-out tree, shared by Assay, which reports them, and the runner, which answers Scope and marks them on the window.

use std::collections::HashMap;

use crate::grid::Area;
use crate::hit::Hits;
use crate::tree::{Kind, NONE, Tree};

/// A taste law that judges one laid-out tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Law {
    /// Every box lies inside its parent and the window.
    OnTheGrid,
    /// Every control has at least one cell where the pointer reaches it.
    Reachable,
    /// No two items in one list read the same.
    Named,
}

impl Law {
    pub const ALL: [Law; 3] = [Law::OnTheGrid, Law::Reachable, Law::Named];

    /// The law in a few words.
    pub fn name(self) -> &'static str {
        match self {
            Law::OnTheGrid => "on the grid",
            Law::Reachable => "reachable",
            Law::Named => "told apart",
        }
    }
}

/// A box that breaks a taste law, and what is wrong, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flaw {
    pub law: Law,
    pub node: u32,
    pub said: String,
}

/// Every box that breaks a taste law, law by law.
pub fn judge<C>(tree: &Tree<C>, window: Area) -> Vec<Flaw> {
    let mut flaws = off_grid(tree, window);
    flaws.extend(unreachable(tree, window));
    flaws.extend(same_names(tree));
    flaws
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
        flaws.push(Flaw { law: Law::OnTheGrid, node: i as u32, said });
    }
    flaws
}

/// Every control on screen that the pointer can't reach in any of its cells, because others lie over all of them. Controls under an open menu are not judged, since the menu is meant to cover them; nor are controls of one kind that share a parent and its whole area, like a range's two handles, since the kernel chooses between them by where the pointer is.
pub fn unreachable<C>(tree: &Tree<C>, window: Area) -> Vec<Flaw> {
    let mut hits = Hits::default();
    hits.fill(tree, window.cols, window.rows);
    let covered = |col: i32, row: i32| hits.at(col, row).is_none() && tree.nodes.iter().any(|n| matches!(n.kind, Kind::Overlay { .. }) && n.area.contains(col, row));
    let mut flaws = Vec::new();
    for (i, n) in tree.nodes.iter().enumerate() {
        if !n.kind.acts() || n.area.is_empty() || matches!(n.kind, Kind::Overlay { .. }) {
            continue;
        }
        let a = n.area;
        let mut reached = false;
        let mut under_menu = false;
        'cells: for row in a.row.max(0)..a.bottom().min(window.rows) {
            for col in a.col.max(0)..a.right().min(window.cols) {
                match hits.at(col, row) {
                    Some(at) if at == i as u32 || shares(tree, at, i as u32) => {
                        reached = true;
                        break 'cells;
                    }
                    None if !n.over && covered(col, row) => under_menu = true,
                    _ => {}
                }
            }
        }
        if !reached && !under_menu {
            let over = (a.row..a.bottom()).find_map(|row| (a.col..a.right()).find_map(|col| hits.at(col, row))).map(|o| tree.nodes[o as usize].kind.role()).unwrap_or("nothing");
            flaws.push(Flaw { law: Law::Reachable, node: i as u32, said: format!("a {} {} can't be reached by the pointer: a {over} lies over all of it", n.kind.role(), at_cells(a)) });
        }
    }
    flaws
}

fn shares<C>(tree: &Tree<C>, a: u32, b: u32) -> bool {
    let (a, b) = (tree.nodes[a as usize], tree.nodes[b as usize]);
    a.parent == b.parent && a.area == b.area && std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind)
}

/// Every item of a list that reads the same as an earlier item of that list, read the way a screen reader reads an item: all the text inside it, in order.
pub fn same_names<C>(tree: &Tree<C>) -> Vec<Flaw> {
    let mut flaws = Vec::new();
    for list in 0..tree.nodes.len() as u32 {
        if !matches!(tree.nodes[list as usize].kind, Kind::List { .. }) {
            continue;
        }
        let mut seen: HashMap<String, u32> = HashMap::new();
        for row in tree.children(list).filter(|&r| matches!(tree.nodes[r as usize].kind, Kind::ListRow { .. })) {
            let mut words = Vec::new();
            read(tree, row, &mut words);
            let name = words.join(" ");
            if name.is_empty() {
                continue;
            }
            if seen.insert(name.clone(), row).is_some() {
                let n = tree.nodes[row as usize];
                flaws.push(Flaw { law: Law::Named, node: row, said: format!("two items of one list both read “{name}”; the second is {}", at_cells(n.area)) });
            }
        }
    }
    flaws
}

fn read<C>(tree: &Tree<C>, i: u32, words: &mut Vec<String>) {
    let n = tree.nodes[i as usize];
    let text = tree.str(n.text).trim();
    if !text.is_empty() {
        words.push(text.to_string());
    }
    for c in tree.children(i) {
        read(tree, c, words);
    }
}

fn at_cells(a: Area) -> String {
    format!("at column {}, row {}, {} by {} cells", a.col, a.row, a.cols, a.rows)
}
