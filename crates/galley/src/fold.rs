use std::ops::Range;

use ledger::{Field, Value};

use crate::changes::Changes;
use crate::doc::Doc;
use crate::find::Stale;

/// Lines folded away, each fold hidden under the line before it until it is unfolded. Each is kept as the characters from the start of its first hidden line to the end of its last, in order and never overlapping, so edits carry it as they carry the cursors; an edit that reaches into a fold opens it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folds(Vec<Range<usize>>);

impl Folds {
    /// The folded ranges of characters, in order.
    pub fn ranges(&self) -> &[Range<usize>] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn map(&mut self, changes: &Changes) {
        self.0.retain_mut(|fold| {
            let mut shift = 0isize;
            for (range, text) in changes.iter() {
                if range.end < fold.start {
                    shift += text.chars().count() as isize - range.len() as isize;
                } else if range.start <= fold.end {
                    return false;
                } else {
                    break;
                }
            }
            fold.start = fold.start.saturating_add_signed(shift);
            fold.end = fold.end.saturating_add_signed(shift);
            true
        });
    }

    pub(crate) fn value_into(&self, value: &mut Value) {
        if let Value::Map(fields) = value
            && !self.0.is_empty()
        {
            let folds = self.0.iter().map(|f| Value::List(vec![Value::UInt(f.start as u64), Value::UInt(f.end as u64)]));
            fields.push(("folds".into(), Value::List(folds.collect())));
        }
    }

    pub(crate) fn from_value(value: &Value) -> Option<Folds> {
        let Some(Value::List(folds)) = value.field("folds") else { return Some(Folds::default()) };
        let ranges = folds
            .iter()
            .map(|f| match f {
                Value::List(p) => match p.as_slice() {
                    [Value::UInt(s), Value::UInt(e)] if s <= e => Some(*s as usize..*e as usize),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        ranges.windows(2).all(|w| w[0].end < w[1].start).then_some(Folds(ranges))
    }
}

impl Doc {
    /// The lines folded away.
    pub fn folds(&self) -> &Folds {
        &self.folds
    }

    /// How many lines are folded away under `line`.
    pub fn folded(&self, line: usize) -> usize {
        let Some(next) = line.checked_add(1).filter(|&l| l < self.lines()) else { return 0 };
        let start = self.line(next).start;
        match self.folds.0.binary_search_by_key(&start, |f| f.start) {
            Ok(k) => self.line_of(self.folds.0[k].end) - line,
            Err(_) => 0,
        }
    }

    /// The line that shows for `line`: itself, or the line it is folded away under.
    pub fn shown(&self, line: usize) -> usize {
        let at = self.line(line).start;
        match self.fold_holding(at) {
            Some(k) => self.line_of(self.folds.0[k].start) - 1,
            None => line,
        }
    }

    fn fold_holding(&self, at: usize) -> Option<usize> {
        let k = self.folds.0.partition_point(|f| f.start <= at);
        k.checked_sub(1).filter(|&k| self.folds.0[k].end >= at)
    }

    /// Folds away `lines` under the line before them; any fold they reach into becomes part of this one. The first line can't be folded, since nothing is above it to hold it.
    pub fn fold(&mut self, lines: Range<usize>) -> bool {
        let last = lines.end.min(self.lines());
        if lines.start == 0 || lines.start >= last {
            return false;
        }
        let mut range = self.line(lines.start).start..self.line(last - 1).end;
        let folds = &mut self.folds.0;
        let from = folds.partition_point(|f| f.end + 1 < range.start);
        let to = folds.partition_point(|f| f.start <= range.end + 1);
        if from < to {
            range.start = range.start.min(folds[from].start);
            range.end = range.end.max(folds[to - 1].end);
        }
        folds.splice(from..to, [range]);
        self.open_at_cursors();
        true
    }

    /// The block to fold at the primary cursor: the lines after its line indented further, or else the block its line belongs to; `None` when nothing is indented there. A block may run to the end of the text, so run it on a copy in a job.
    pub fn block(&self) -> Option<Block> {
        let line = self.line_of(self.selections().primary().head);
        let lines = self.block_under(line).or_else(|| self.block_holding(line))?;
        Some(Block { first: lines.start, end: lines.end, revision: Some(self.revision()) })
    }

    /// Folds away the block found, and returns how many lines it hid. Refused when the text changed after it was found.
    pub fn fold_found(&mut self, block: &Block) -> Result<usize, Stale> {
        if block.revision.is_some_and(|r| r != self.revision()) || block.end > self.lines() {
            return Err(Stale);
        }
        Ok(if self.fold(block.first..block.end) { block.end - block.first } else { 0 })
    }

    fn block_under(&self, line: usize) -> Option<Range<usize>> {
        let indent = self.indent(line)?;
        let mut end = line + 1;
        let mut last = None;
        while end < self.lines() {
            match self.indent(end) {
                Some(i) if i <= indent => break,
                Some(_) => last = Some(end),
                None => {}
            }
            end += 1;
        }
        last.map(|last| line + 1..last + 1)
    }

    fn block_holding(&self, line: usize) -> Option<Range<usize>> {
        let indent = self.indent(line).unwrap_or(usize::MAX);
        let header = (0..line).rev().find(|&l| self.indent(l).is_some_and(|i| i < indent))?;
        self.block_under(header)
    }

    fn indent(&self, line: usize) -> Option<usize> {
        let range = self.line(line);
        let mut cells = 0;
        for at in range {
            match self.char(at) {
                Some(' ') => cells += 1,
                Some('\t') => cells += 4,
                _ => return Some(cells),
            }
        }
        None
    }

    /// Unfolds what is folded under the primary cursor's line, and returns how many lines show again.
    pub fn unfold(&mut self) -> usize {
        let line = self.line_of(self.selections().primary().head);
        let count = self.folded(line);
        if count > 0 {
            let start = self.line(line + 1).start;
            self.folds.0.retain(|f| f.start != start);
        }
        count
    }

    /// Unfolds everything, and returns how many folds there were.
    pub fn unfold_all(&mut self) -> usize {
        std::mem::take(&mut self.folds.0).len()
    }

    pub(crate) fn open_at_cursors(&mut self) {
        if self.folds.0.is_empty() {
            return;
        }
        let heads: Vec<usize> = self.selections().spans().iter().map(|s| s.head).collect();
        let holding: Vec<usize> = heads.into_iter().filter_map(|h| self.fold_holding(h)).collect();
        if holding.is_empty() {
            return;
        }
        let mut k = 0;
        self.folds.0.retain(|_| {
            k += 1;
            !holding.contains(&(k - 1))
        });
    }
}

/// The lines of a block to fold, worked out on a copy of a [`Doc`] by [`Doc::block`] and folded by [`Doc::fold_found`]. It travels as a command's argument, so the command log keeps which lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    first: usize,
    end: usize,
    revision: Option<u64>,
}

impl Field for Block {
    ledger::register!(Block);

    fn same(&self, other: &Self) -> bool {
        (self.first, self.end) == (other.first, other.end)
    }

    fn value(&self) -> Value {
        Value::Map(vec![("first".into(), Value::UInt(self.first as u64)), ("end".into(), Value::UInt(self.end as u64))])
    }

    fn from_value(value: &Value) -> Option<Self> {
        let (Some(Value::UInt(first)), Some(Value::UInt(end))) = (value.field("first"), value.field("end")) else { return None };
        Some(Block { first: *first as usize, end: *end as usize, revision: None })
    }
}
