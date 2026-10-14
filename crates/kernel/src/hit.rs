use crate::tree::{NONE, Tree};

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
        for (i, n) in tree.nodes.iter().enumerate() {
            if !n.kind.acts() || n.area.is_empty() {
                continue;
            }
            let a = n.area;
            for row in a.row.max(0)..a.bottom().min(self.rows) {
                let line = (row * self.cols) as usize;
                for col in a.col.max(0)..a.right().min(self.cols) {
                    self.cells[line + col as usize] = i as u32;
                }
            }
        }
    }

    /// The node that acts at this cell, if any.
    pub fn at(&self, col: i32, row: i32) -> Option<u32> {
        if col < 0 || row < 0 || col >= self.cols || row >= self.rows {
            return None;
        }
        let i = self.cells[(row * self.cols + col) as usize];
        (i != NONE).then_some(i)
    }
}
