use ropey::RopeSlice;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use ledger::wrap;

use crate::doc::Doc;

/// A way to move through text. Left and right step over whole characters as a reader sees them, so an accented letter or an emoji with its modifiers is one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    Left,
    Right,
    /// To the start of the next word.
    WordNext,
    /// To the start of this word, or the one before.
    WordPrevious,
    /// To the end of this word, or the next.
    WordEnd,
    LineStart,
    /// To the first character of the line that isn't a space.
    LineIndent,
    LineEnd,
    Up,
    Down,
    /// Up by one of the rows long lines wrap into at this many cells, keeping the cursor's column in cells.
    RowUp(u16),
    RowDown(u16),
    /// To the blank line after this paragraph.
    ParagraphNext,
    /// To the blank line before this paragraph.
    ParagraphPrevious,
    Start,
    End,
}

impl Doc {
    /// Where `motion` takes a cursor at `at`.
    pub fn motion(&self, at: usize, motion: Motion) -> usize {
        let at = at.min(self.len());
        match motion {
            Motion::Left => before(self, at),
            Motion::Right => after(self, at),
            Motion::WordNext => {
                let mut i = at;
                let class = self.char(i).map(Class::of);
                while i < self.len() && self.char(i).map(Class::of) == class && class != Some(Class::Space) {
                    i += 1;
                }
                while i < self.len() && self.char(i).map(Class::of) == Some(Class::Space) {
                    i += 1;
                }
                i
            }
            Motion::WordPrevious => {
                let mut i = at;
                while i > 0 && self.char(i - 1).map(Class::of) == Some(Class::Space) {
                    i -= 1;
                }
                let class = i.checked_sub(1).and_then(|p| self.char(p)).map(Class::of);
                while i > 0 && self.char(i - 1).map(Class::of) == class {
                    i -= 1;
                }
                i
            }
            Motion::WordEnd => {
                let mut i = at;
                while i < self.len() && self.char(i).map(Class::of) == Some(Class::Space) {
                    i += 1;
                }
                let class = self.char(i).map(Class::of);
                while i < self.len() && self.char(i).map(Class::of) == class {
                    i += 1;
                }
                i
            }
            Motion::LineStart => self.line(self.line_of(at)).start,
            Motion::LineIndent => {
                let line = self.line(self.line_of(at));
                (line.start..line.end).find(|&i| !matches!(self.char(i), Some(' ' | '\t'))).unwrap_or(line.end)
            }
            Motion::LineEnd => self.line(self.line_of(at)).end,
            Motion::Up | Motion::Down => {
                let line = self.line_of(at);
                let column = at - self.line(line).start;
                let target = match motion {
                    Motion::Up if line == 0 => return 0,
                    Motion::Up => line - 1,
                    _ if line + 1 >= self.lines() => return self.len(),
                    _ => line + 1,
                };
                let range = self.line(target);
                (range.start + column).min(range.end)
            }
            Motion::RowUp(width) => self.row(at, usize::from(width), false),
            Motion::RowDown(width) => self.row(at, usize::from(width), true),
            Motion::ParagraphNext => {
                let mut line = self.line_of(at);
                while line < self.lines() && self.blank(line) {
                    line += 1;
                }
                while line < self.lines() && !self.blank(line) {
                    line += 1;
                }
                if line >= self.lines() { self.len() } else { self.line(line).start }
            }
            Motion::ParagraphPrevious => {
                let mut line = self.line_of(at);
                if at == self.line(line).start && line > 0 {
                    line -= 1;
                }
                while line > 0 && self.blank(line) {
                    line -= 1;
                }
                while line > 0 && !self.blank(line) {
                    line -= 1;
                }
                self.line(line).start
            }
            Motion::Start => 0,
            Motion::End => self.len(),
        }
    }

    fn row(&self, at: usize, width: usize, down: bool) -> usize {
        let line = self.line_of(at);
        let span = self.line(line);
        let text = self.text(span.clone());
        let byte = byte_of(&text, at - span.start);
        let (k, row) = wrap::row_of(&text, width, byte);
        let cells = wrap::cells_to(&text, &row, byte);
        let count = wrap::rows(&text, width).count();
        let (line, k) = match (down, k) {
            (false, 0) if line == 0 => return 0,
            (false, 0) => (line - 1, usize::MAX),
            (false, k) => (line, k - 1),
            (true, k) if k + 1 < count => (line, k + 1),
            (true, _) if line + 1 >= self.lines() => return self.len(),
            (true, _) => (line + 1, 0),
        };
        let span = self.line(line);
        let text = self.text(span.clone());
        let Some(row) = wrap::rows(&text, width).take(k.saturating_add(1)).last() else { return span.start };
        span.start + text[..wrap::at_cells(&text, &row, cells)].chars().count()
    }

    fn blank(&self, line: usize) -> bool {
        let range = self.line(line);
        (range.start..range.end).all(|i| matches!(self.char(i), Some(' ' | '\t')))
    }
}

fn byte_of(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(b, _)| b)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Mark,
}

impl Class {
    fn of(c: char) -> Class {
        if c.is_whitespace() {
            Class::Space
        } else if c.is_alphanumeric() || c == '_' {
            Class::Word
        } else {
            Class::Mark
        }
    }
}

/// The start of the character as a reader sees it that ends at `at`.
pub(crate) fn before(doc: &Doc, at: usize) -> usize {
    boundary(doc.rope().slice(..), at, false)
}

/// The end of the character as a reader sees it that starts at `at`.
pub(crate) fn after(doc: &Doc, at: usize) -> usize {
    boundary(doc.rope().slice(..), at, true)
}

fn boundary(text: RopeSlice, at: usize, forward: bool) -> usize {
    let at = at.min(text.len_chars());
    let fallback = if forward { (at + 1).min(text.len_chars()) } else { at.saturating_sub(1) };
    let byte = text.char_to_byte(at);
    let (mut chunk, mut chunk_start, _, _) = text.chunk_at_byte(byte);
    let mut cursor = GraphemeCursor::new(byte, text.len_bytes(), true);
    for _ in 0..64 {
        let found = if forward { cursor.next_boundary(chunk, chunk_start) } else { cursor.prev_boundary(chunk, chunk_start) };
        match found {
            Ok(Some(b)) => return text.byte_to_char(b),
            Ok(None) => return if forward { text.len_chars() } else { 0 },
            Err(GraphemeIncomplete::NextChunk) => (chunk, chunk_start, _, _) = text.chunk_at_byte(chunk_start + chunk.len()),
            Err(GraphemeIncomplete::PrevChunk) => (chunk, chunk_start, _, _) = text.chunk_at_byte(chunk_start - 1),
            Err(GraphemeIncomplete::PreContext(n)) => {
                let (context, context_start, _, _) = text.chunk_at_byte(n - 1);
                cursor.provide_context(context, context_start);
            }
            Err(GraphemeIncomplete::InvalidOffset) => return fallback,
        }
    }
    fallback
}
