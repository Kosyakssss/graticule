//! Where text wraps in monospace cells: the rule the kernel draws long lines by and the text an editor shows moves its cursors by, so moving up a row lands where the row above is drawn.

use std::ops::Range;

/// How many cells a character takes in mono text: two for Chinese, Japanese and Korean, as in a terminal.
pub fn cells(c: char) -> usize {
    match c as u32 {
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

/// The rows a text wraps into at `width` cells, as byte ranges: a new line ends a row, and a line too long for the width breaks after its last space that fits, or mid-word when there is none. A row that wraps keeps its space, so the rows of a line touch.
pub fn rows(text: &str, width: usize) -> Rows<'_> {
    Rows { text, width: width.max(1), at: 0, done: false }
}

/// The rows of a text, from [`rows`].
pub struct Rows<'a> {
    text: &'a str,
    width: usize,
    at: usize,
    done: bool,
}

impl Iterator for Rows<'_> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Range<usize>> {
        if self.done {
            return None;
        }
        let at = self.at;
        let line_end = self.text[at..].find('\n').map_or(self.text.len(), |k| at + k);
        let (mut used, mut space) = (0, None);
        for (k, c) in self.text[at..line_end].char_indices() {
            let w = cells(c);
            if used + w > self.width && k > 0 {
                let end = space.unwrap_or(at + k);
                self.at = end;
                return Some(at..end);
            }
            used += w;
            if c == ' ' {
                space = Some(at + k + 1);
            }
        }
        if line_end == self.text.len() {
            self.done = true;
        } else {
            self.at = line_end + 1;
        }
        Some(at..line_end)
    }
}

/// The row a byte sits on, and that row: the last row that starts at or before it.
pub fn row_of(text: &str, width: usize, at: usize) -> (usize, Range<usize>) {
    let mut found = (0, 0..0);
    for (k, row) in rows(text, width).enumerate() {
        if row.start > at {
            break;
        }
        found = (k, row);
    }
    found
}

/// How many cells in from the start of `row` the byte `at` is.
pub fn cells_to(text: &str, row: &Range<usize>, at: usize) -> usize {
    text[row.start..at.clamp(row.start, row.end)].chars().map(cells).sum()
}

/// The byte on `row` nearest to `cells` cells in from its start.
pub fn at_cells(text: &str, row: &Range<usize>, cells: usize) -> usize {
    let end = row_end(text, row);
    let mut used = 0;
    for (k, c) in text[row.start..end].char_indices() {
        let w = self::cells(c);
        if used + w / 2 >= cells {
            return row.start + k;
        }
        used += w;
    }
    end
}

/// Where a row really ends for a cursor: before the last character of a row that wraps, since the end of such a row is the start of the next.
pub fn row_end(text: &str, row: &Range<usize>) -> usize {
    if row.end < text.len() && !text[row.end..].starts_with('\n') { text[..row.end].char_indices().next_back().map_or(0, |(i, _)| i) } else { row.end }
}
