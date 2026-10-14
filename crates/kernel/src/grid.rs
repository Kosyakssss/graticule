#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Area {
    pub col: i32,
    pub row: i32,
    pub cols: i32,
    pub rows: i32,
}

impl Area {
    pub const fn new(col: i32, row: i32, cols: i32, rows: i32) -> Self {
        Self { col, row, cols, rows }
    }

    pub const fn right(&self) -> i32 {
        self.col + self.cols
    }

    pub const fn bottom(&self) -> i32 {
        self.row + self.rows
    }

    pub const fn is_empty(&self) -> bool {
        self.cols <= 0 || self.rows <= 0
    }

    pub const fn contains(&self, col: i32, row: i32) -> bool {
        col >= self.col && row >= self.row && col < self.right() && row < self.bottom()
    }

    pub fn take_top(&mut self, rows: i32) -> Area {
        let rows = rows.clamp(0, self.rows.max(0));
        let cut = Area::new(self.col, self.row, self.cols, rows);
        self.row += rows;
        self.rows -= rows;
        cut
    }

    pub fn take_bottom(&mut self, rows: i32) -> Area {
        let rows = rows.clamp(0, self.rows.max(0));
        self.rows -= rows;
        Area::new(self.col, self.bottom(), self.cols, rows)
    }

    pub fn take_left(&mut self, cols: i32) -> Area {
        let cols = cols.clamp(0, self.cols.max(0));
        let cut = Area::new(self.col, self.row, cols, self.rows);
        self.col += cols;
        self.cols -= cols;
        cut
    }

    pub fn take_right(&mut self, cols: i32) -> Area {
        let cols = cols.clamp(0, self.cols.max(0));
        self.cols -= cols;
        Area::new(self.right(), self.row, cols, self.rows)
    }

    pub const fn inset(&self, cols: i32, rows: i32) -> Area {
        Area::new(self.col + cols, self.row + rows, self.cols - cols * 2, self.rows - rows * 2)
    }

    pub const fn line(&self, i: i32) -> Area {
        Area::new(self.col, self.row + i, self.cols, 1)
    }

    pub const fn at(&self, col: i32, row: i32, cols: i32, rows: i32) -> Area {
        Area::new(self.col + col, self.row + row, cols, rows)
    }

    pub fn columns<const N: usize>(&self, weights: [i32; N], gap: i32) -> [Area; N] {
        let total: i32 = weights.iter().sum::<i32>().max(1);
        let room = (self.cols - gap * (N as i32 - 1)).max(0);
        let mut out = [Area::default(); N];
        let mut col = self.col;
        let mut used = 0;
        for (i, w) in weights.iter().enumerate() {
            let cols = if i + 1 == N { room - used } else { room * w / total };
            out[i] = Area::new(col, self.row, cols, self.rows);
            col += cols + gap;
            used += cols;
        }
        out
    }
}
