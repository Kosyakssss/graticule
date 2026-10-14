use crate::grid::Area;
use crate::metrics::Metrics;
use crate::picture::Picture;
use crate::text::{SpanStyle, Text, TextStyle};
use crate::tree::{Kind, NONE, Tree, Width};
use crate::value::Value;

pub const GAP: i32 = 2;
pub const MARGIN: i32 = 2;
pub const CHECK: i32 = 3;
pub const BUTTON: i32 = 3;
pub const SWATCH: i32 = 4;
const BUTTON_MIN: i32 = 16;

/// Lays a tree out in whole cells: each node's `Area` from its parent's, top down, measuring text in columns as it goes.
pub struct Layout<'t> {
    pub text: &'t mut Text,
    pub m: Metrics,
    pub stack: Vec<u32>,
    pub widths: Vec<i32>,
}

impl<'t> Layout<'t> {
    pub fn new(text: &'t mut Text, m: Metrics, stack: Vec<u32>, widths: Vec<i32>) -> Self {
        Self { text, m, stack, widths }
    }

    pub fn finish(self) -> (Vec<u32>, Vec<i32>) {
        (self.stack, self.widths)
    }

    pub fn cols(&mut self, s: &str, mono: bool) -> i32 {
        let style = self.style(mono);
        self.cols_as(s, style)
    }

    fn cols_as(&mut self, s: &str, style: TextStyle) -> i32 {
        let mono = style.face == crate::text::Face::Mono;
        let cells = s.chars().map(crate::text::cells).sum::<usize>() as i32;
        if mono && s.is_ascii() {
            return cells;
        }
        let w = self.text.measure(s, style, self.m.cell_h);
        if mono {
            return cells.max((w / self.m.cell_w - 0.01).ceil() as i32);
        }
        (w / self.m.cell_w - 0.01).ceil().max(0.0) as i32
    }

    fn picture_cols(&self, p: &Picture, rows: i32) -> i32 {
        let height = rows as f32 * self.m.cell_h;
        ((height * p.width / p.height.max(f32::EPSILON)) / self.m.cell_w - 0.01).ceil().max(1.0) as i32
    }

    fn style(&self, mono: bool) -> TextStyle {
        if mono { self.m.mono() } else { self.m.sans(400.0) }
    }

    fn text_style(&self, kind: Kind) -> TextStyle {
        match kind {
            Kind::Text { strong: true, mono: false, .. } => self.m.sans(600.0),
            Kind::Text { mono, .. } => self.style(mono),
            _ => self.style(false),
        }
    }

    /// How many rows `s` takes when wrapped at `cols` columns.
    pub fn wrapped(&mut self, s: &str, mono: bool, cols: i32) -> i32 {
        let style = self.style(mono);
        self.wrapped_as(s, style, cols)
    }

    fn wrapped_as(&mut self, s: &str, style: TextStyle, cols: i32) -> i32 {
        if s.contains('\n') {
            return s.split('\n').map(|line| self.wrapped_as(line, style, cols)).sum();
        }
        if s.is_empty() {
            return 1;
        }
        let n = self.cols_as(s, style);
        if style.face == crate::text::Face::Mono {
            return ((n + cols.max(1) - 1) / cols.max(1)).max(1);
        }
        if n <= cols {
            return 1;
        }
        let span = SpanStyle { weight: style.weight as u16, ..SpanStyle::plain(style.face) };
        let h = self.text.height(&[(s, span)], style.size, self.m.cell_h, cols.max(1) as f32 * self.m.cell_w);
        ((h / self.m.cell_h).round() as i32).max(1)
    }

    fn children<C>(&mut self, tree: &Tree<C>, i: u32) -> (usize, usize) {
        let start = self.stack.len();
        let mut c = tree.nodes[i as usize].first;
        while c != NONE {
            self.stack.push(c);
            c = tree.nodes[c as usize].next;
        }
        (start, self.stack.len())
    }

    fn release(&mut self, start: usize) {
        self.stack.truncate(start);
    }

    /// The fewest columns a node can take, and the number it would like.
    pub fn size<C>(&mut self, tree: &Tree<C>, i: u32) -> (i32, i32) {
        let n = tree.nodes[i as usize];
        let (min, pref) = match n.kind {
            Kind::Text { .. } => {
                let style = self.text_style(n.kind);
                let pref = self.cols_as(tree.str(n.text), style);
                (pref.min(8), pref)
            }
            Kind::Button => {
                let c = self.cols(tree.str(n.text), true) + BUTTON;
                (c.min(BUTTON_MIN + BUTTON), c)
            }
            Kind::Action | Kind::Choice { .. } => {
                let keys = tree.str(n.keys);
                let keys = if keys.is_empty() { 0 } else { self.cols(keys, true) + 1 };
                let c = self.cols(tree.str(n.text), false) + 2 + keys;
                (c.min(if matches!(n.kind, Kind::Choice { .. }) { 16 } else { c }), c)
            }
            Kind::Link => {
                let c = self.cols(tree.str(n.text), false).max(1);
                (c, c)
            }
            Kind::Check { .. } => {
                let label = self.cols(tree.str(n.text), false);
                (CHECK + 1 + label.min(8), CHECK + 1 + label)
            }
            Kind::Value { value: Value::Error } => {
                let pref = self.cols(tree.str(n.text), false);
                (pref.min(8), pref)
            }
            Kind::Value { value } => {
                let c = self.cols(tree.str(n.text), true) + i32::from(value.boxed());
                (c.min(8), c)
            }
            Kind::Field => (16, 32),
            Kind::Swatch { .. } => (SWATCH, SWATCH),
            Kind::Picture { picture, rows } => {
                let c = self.picture_cols(picture, rows);
                (c, c)
            }
            Kind::Row | Kind::ListRow { .. } | Kind::Columns => {
                let (start, end) = self.children(tree, i);
                let (mut min, mut pref, mut count) = (0, 0, 0);
                for k in start..end {
                    let (a, b) = self.size(tree, self.stack[k]);
                    min += a;
                    pref += b;
                    count += 1;
                }
                self.release(start);
                let gaps = GAP * (count - 1).max(0);
                (min + gaps, pref + gaps)
            }
            Kind::Root | Kind::Page | Kind::Column | Kind::Empty | Kind::Bar | Kind::Palette | Kind::List { .. } => {
                let (start, end) = self.children(tree, i);
                let (mut min, mut pref) = (0, 0);
                for k in start..end {
                    let (a, b) = self.size(tree, self.stack[k]);
                    min = min.max(a);
                    pref = pref.max(b);
                }
                self.release(start);
                match n.kind {
                    Kind::Page => (min + MARGIN * 2, pref + MARGIN * 2),
                    Kind::List { .. } => (min + 2, pref + 2),
                    _ => (min, pref),
                }
            }
            Kind::Status | Kind::Note => (0, self.cols(tree.str(n.text), false)),
        };
        let span = n.span;
        match span.width {
            Width::Auto => (min.max(span.min).min(span.max), pref.max(span.min).min(span.max)),
            Width::Cols(c) => (span.min.min(c), c),
            Width::Fill(_) => (min.max(span.min), pref.max(span.min).min(span.max)),
        }
    }

    /// Whether a row of children fits side by side in `cols`, or stacks.
    fn fits<C>(&mut self, tree: &Tree<C>, i: u32, cols: i32) -> bool {
        self.size(tree, i).0 <= cols
    }

    /// The widths of a fitting row's children, pushed onto `widths` in order.
    fn share<C>(&mut self, tree: &Tree<C>, start: usize, end: usize, cols: i32) -> usize {
        let at = self.widths.len();
        let count = (end - start) as i32;
        let room = cols - GAP * (count - 1).max(0);
        let mut used = 0;
        let mut weight = 0u32;
        for k in start..end {
            let c = self.stack[k];
            let (min, pref) = self.size(tree, c);
            let w = match tree.nodes[c as usize].span.width {
                Width::Fill(n) => {
                    weight += n as u32;
                    min
                }
                _ => pref,
            };
            self.widths.push(w);
            used += w;
        }
        if used > room {
            let mut over = used - room;
            for k in (start..end).rev() {
                if over <= 0 {
                    break;
                }
                let (min, _) = self.size(tree, self.stack[k]);
                let w = &mut self.widths[at + k - start];
                let cut = (*w - min).min(over).max(0);
                *w -= cut;
                over -= cut;
            }
        } else if weight > 0 {
            let fill = |k: usize| match tree.nodes[self.stack[k] as usize].span.width {
                Width::Fill(n) => Some(n as u32),
                _ => None,
            };
            let mut pool = room;
            for k in start..end {
                let w = &mut self.widths[at + k - start];
                if fill(k).is_some() {
                    *w = -(*w + 1);
                } else {
                    pool -= *w;
                }
            }
            let mut changed = true;
            while changed {
                changed = false;
                for k in start..end {
                    let w = self.widths[at + k - start];
                    let Some(n) = fill(k).filter(|_| w < 0) else { continue };
                    let min = -w - 1;
                    if (pool as i64 * n as i64) < (min as i64 * weight as i64) {
                        self.widths[at + k - start] = min;
                        pool -= min;
                        weight -= n;
                        changed = true;
                    }
                }
            }
            let (mut given, mut seen) = (0, 0u32);
            for k in start..end {
                let w = &mut self.widths[at + k - start];
                let Some(n) = fill(k).filter(|_| *w < 0) else { continue };
                seen += n;
                let share = (pool as i64 * seen as i64 / weight as i64) as i32 - given;
                given += share;
                *w = share.min(tree.nodes[self.stack[k] as usize].span.max);
            }
        }
        at
    }

    /// How many rows a node takes at `cols` columns wide.
    pub fn rows<C>(&mut self, tree: &Tree<C>, i: u32, cols: i32) -> i32 {
        let n = tree.nodes[i as usize];
        match n.kind {
            Kind::Text { .. } => {
                let style = self.text_style(n.kind);
                self.wrapped_as(tree.str(n.text), style, cols)
            }
            Kind::Check { .. } => self.wrapped(tree.str(n.text), false, cols - CHECK - 1),
            Kind::Value { value: Value::Error } => self.wrapped(tree.str(n.text), false, cols),
            Kind::Value { .. } => 1,
            Kind::Button | Kind::Action | Kind::Choice { .. } | Kind::Link | Kind::Field | Kind::Status | Kind::Note | Kind::Swatch { .. } => 1,
            Kind::Picture { rows, .. } => rows,
            Kind::List { .. } => 3,
            Kind::Bar => {
                let (start, end) = self.children(tree, i);
                let (mut rows, mut col) = (1, 0);
                for k in start..end {
                    let w = self.size(tree, self.stack[k]).1;
                    if col > 0 && col + w > cols {
                        rows += 1;
                        col = 0;
                    }
                    col += w + GAP;
                }
                self.release(start);
                rows
            }
            Kind::Row | Kind::ListRow { .. } | Kind::Columns => {
                let (start, end) = self.children(tree, i);
                let rows = if self.fits(tree, i, cols) {
                    let at = self.share(tree, start, end, cols);
                    let mut rows = 1;
                    for k in start..end {
                        let w = self.widths[at + k - start];
                        rows = rows.max(self.rows(tree, self.stack[k], w));
                    }
                    self.widths.truncate(at);
                    rows
                } else {
                    let mut rows = 0;
                    for k in start..end {
                        let c = self.stack[k];
                        let w = self.size(tree, c).1.min(cols);
                        rows += self.rows(tree, c, w);
                    }
                    rows
                };
                self.release(start);
                rows
            }
            Kind::Root | Kind::Page | Kind::Column | Kind::Empty | Kind::Palette => {
                let (start, end) = self.children(tree, i);
                let inner = if n.kind == Kind::Page { cols - MARGIN * 2 } else { cols };
                let gap = i32::from(matches!(n.kind, Kind::Page | Kind::Empty));
                let mut rows = 0;
                for k in start..end {
                    rows += self.rows(tree, self.stack[k], inner) + gap;
                }
                self.release(start);
                if n.kind == Kind::Page { rows + 3 } else { (rows - gap).max(0) }
            }
        }
    }

    /// Lays out the tree inside `screen`: the page above the window's own lines at the bottom.
    pub fn run<C>(&mut self, tree: &mut Tree<C>) {
        let screen = self.m.screen();
        tree.nodes[0].area = screen;
        let mut rest = screen;
        let (start, end) = self.children(tree, 0);
        let mut page = NONE;
        for k in (start..end).rev() {
            let c = self.stack[k];
            match tree.nodes[c as usize].kind {
                Kind::Bar => {
                    let rows = self.rows(tree, c, rest.cols - MARGIN * 2);
                    let a = rest.take_bottom(rows);
                    self.place(tree, c, Area::new(a.col + MARGIN, a.row, a.cols - MARGIN * 2, a.rows));
                }
                Kind::Palette => {
                    let rows = self.rows(tree, c, rest.cols - MARGIN * 2).min(rest.rows / 2);
                    let a = rest.take_bottom(rows);
                    self.place(tree, c, Area::new(a.col + MARGIN, a.row, a.cols - MARGIN * 2, a.rows));
                }
                Kind::Status | Kind::Note => {
                    let a = rest.take_bottom(1);
                    self.place(tree, c, Area::new(a.col + MARGIN, a.row, a.cols - MARGIN * 2, a.rows));
                }
                _ => page = c,
            }
        }
        if page != NONE {
            self.place(tree, page, rest);
        }
        self.release(start);
    }

    pub fn place<C>(&mut self, tree: &mut Tree<C>, i: u32, area: Area) {
        let area = Area::new(area.col, area.row, area.cols.max(0), area.rows.max(0));
        tree.nodes[i as usize].area = area;
        let kind = tree.nodes[i as usize].kind;
        match kind {
            Kind::Page => {
                let inner = Area::new(area.col + MARGIN, area.row + 3, area.cols - MARGIN * 2, area.rows - 4);
                self.stack_down(tree, i, inner, 1);
            }
            Kind::Root | Kind::Column | Kind::Palette => self.stack_down(tree, i, area, 0),
            Kind::Empty => self.stack_down(tree, i, area, 1),
            Kind::Row | Kind::ListRow { .. } | Kind::Columns => self.side_by_side(tree, i, area),
            Kind::List { .. } => self.list(tree, i, area),
            Kind::Bar => {
                let (start, end) = self.children(tree, i);
                let (mut row, mut col) = (0, 0);
                for k in start..end {
                    let c = self.stack[k];
                    let w = self.size(tree, c).1;
                    if col > 0 && col + w > area.cols {
                        row += 1;
                        col = 0;
                    }
                    let a = if row < area.rows { Area::new(area.col + col, area.row + row, w.min(area.cols - col), 1) } else { Area::default() };
                    self.place(tree, c, a);
                    col += w + GAP;
                }
                self.release(start);
            }
            _ => {}
        }
    }

    fn stack_down<C>(&mut self, tree: &mut Tree<C>, i: u32, area: Area, gap: i32) {
        let (start, end) = self.children(tree, i);
        let mut fixed = 0;
        let mut fills = 0;
        for k in start..end {
            let c = self.stack[k];
            if matches!(tree.nodes[c as usize].kind, Kind::List { .. }) {
                fills += 1;
            } else {
                fixed += self.rows(tree, c, area.cols);
            }
        }
        let gaps = gap * (end - start).saturating_sub(1) as i32;
        let spare = (area.rows - fixed - gaps).max(0);
        let mut row = area.row;
        let mut given = 0;
        let mut seen = 0;
        for k in start..end {
            let c = self.stack[k];
            let rows = if matches!(tree.nodes[c as usize].kind, Kind::List { .. }) {
                seen += 1;
                let share = spare * seen / fills.max(1) - given;
                given += share;
                share
            } else {
                self.rows(tree, c, area.cols)
            };
            let rows = rows.min(area.bottom() - row).max(0);
            let cols = if matches!(tree.nodes[c as usize].kind, Kind::Button | Kind::Link | Kind::Swatch { .. } | Kind::Picture { .. }) { self.size(tree, c).1.min(area.cols) } else { area.cols };
            let a = if rows > 0 { Area::new(area.col, row, cols, rows) } else { Area::default() };
            self.place(tree, c, a);
            row += rows + gap;
        }
        self.release(start);
    }

    fn side_by_side<C>(&mut self, tree: &mut Tree<C>, i: u32, area: Area) {
        let (start, end) = self.children(tree, i);
        if self.fits(tree, i, area.cols) {
            let at = self.share(tree, start, end, area.cols);
            let mut col = area.col;
            for k in start..end {
                let c = self.stack[k];
                let w = self.widths[at + k - start];
                let rows = self.rows(tree, c, w).min(area.rows);
                self.place(tree, c, Area::new(col, area.row, w, rows));
                col += w + GAP;
            }
            self.widths.truncate(at);
        } else {
            let mut row = area.row;
            for k in start..end {
                let c = self.stack[k];
                let w = self.size(tree, c).1.min(area.cols);
                let rows = self.rows(tree, c, w).min(area.bottom() - row).max(0);
                let a = if rows > 0 { Area::new(area.col, row, w, rows) } else { Area::default() };
                self.place(tree, c, a);
                row += rows;
            }
        }
        self.release(start);
    }

    fn list<C>(&mut self, tree: &mut Tree<C>, i: u32, area: Area) {
        let (start, end) = self.children(tree, i);
        let cols = area.cols - 2;
        let mut row = area.row;
        for k in start..end {
            let c = self.stack[k];
            let rows = self.rows(tree, c, cols);
            let a = if row + rows <= area.bottom() {
                Area::new(area.col, row, cols, rows)
            } else if row == area.row {
                Area::new(area.col, row, cols, area.rows)
            } else {
                Area::default()
            };
            self.place(tree, c, a);
            row += rows;
        }
        self.release(start);
    }
}

impl Layout<'_> {
    /// The smallest window, in cells, that shows everything in its narrow form: the page's minimum width, and its height at that width with every list three rows tall.
    pub fn minimum<C>(&mut self, tree: &Tree<C>) -> (i32, i32) {
        let (start, end) = self.children(tree, 0);
        let mut cols = 0;
        for k in start..end {
            let c = self.stack[k];
            if !matches!(tree.nodes[c as usize].kind, Kind::Bar | Kind::Status | Kind::Note) {
                cols = cols.max(self.size(tree, c).0);
            }
        }
        cols = cols.max(24);
        let mut rows = 0;
        for k in start..end {
            let c = self.stack[k];
            rows += match tree.nodes[c as usize].kind {
                Kind::Bar => self.rows(tree, c, cols - MARGIN * 2),
                _ => self.rows(tree, c, cols),
            };
        }
        self.release(start);
        (cols, rows)
    }
}
