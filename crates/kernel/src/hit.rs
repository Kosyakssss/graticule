use crate::tree::{Kind, NONE, Tree};

const COVERED: u32 = NONE - 1;

/// The window in cells, each holding the topmost node that acts there. Finding what is under the pointer is one read.
#[derive(Default)]
pub struct Hits {
    cols: i32,
    rows: i32,
    cells: Vec<u32>,
}

impl Hits {
    pub fn fill<C>(&mut self, tree: &Tree<C>, cols: i32, rows: i32) {
        self.cols = cols.max(0);
        self.rows = rows.max(0);
        self.cells.clear();
        self.cells.resize((self.cols * self.rows) as usize, NONE);
        for over in [false, true] {
            for (i, n) in tree.nodes.iter().enumerate() {
                let overlay = matches!(n.kind, Kind::Overlay { .. });
                if n.over != over || !(overlay || n.kind.acts()) || n.area.is_empty() {
                    continue;
                }
                let a = n.area;
                let at = if overlay { COVERED } else { i as u32 };
                for row in a.row.max(0)..a.bottom().min(self.rows) {
                    let line = (row * self.cols) as usize;
                    for col in a.col.max(0)..a.right().min(self.cols) {
                        self.cells[line + col as usize] = at;
                    }
                }
            }
        }
    }

    /// The node that acts at this cell, if any. Inside an overlay, only the overlay's own nodes are found.
    pub fn at(&self, col: i32, row: i32) -> Option<u32> {
        if col < 0 || row < 0 || col >= self.cols || row >= self.rows {
            return None;
        }
        let i = self.cells[(row * self.cols + col) as usize];
        (i < COVERED).then_some(i)
    }
}
