use std::ops::Range;

/// Replacements made at once: each a range of the text as it was, in characters, and what takes its place. Ranges are kept in order and never overlap; one that overlaps an earlier one is dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    each: Vec<(Range<usize>, String)>,
}

impl Changes {
    pub fn new(each: impl IntoIterator<Item = (Range<usize>, String)>) -> Self {
        let mut all: Vec<(Range<usize>, String)> = each.into_iter().filter(|(r, _)| r.start <= r.end).collect();
        all.sort_by_key(|(r, _)| (r.start, r.end));
        let mut each: Vec<(Range<usize>, String)> = Vec::with_capacity(all.len());
        for (range, text) in all {
            if each.last().is_none_or(|(last, _)| last.end <= range.start && !(last.is_empty() && range.is_empty() && last.start == range.start)) {
                each.push((range, text));
            }
        }
        Changes { each }
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &(Range<usize>, String)> {
        self.each.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.each.is_empty()
    }

    /// Where a position in the text as it was ends up after the changes. A position inside a replaced range, or at its end, moves to the end of what replaced it; one at its start stays before it, unless nothing was removed there, so a cursor stays after what is typed at it.
    pub fn map(&self, at: usize) -> usize {
        self.mapper().map(at)
    }

    /// Maps many positions in one pass over the changes, as long as each comes no earlier than the one before.
    pub fn mapper(&self) -> Mapper<'_> {
        Mapper { each: &self.each, next: 0, shift: 0 }
    }
}

/// Maps positions given in order through [`Changes`], each no earlier than the last, so mapping every cursor costs one pass however many there are.
pub struct Mapper<'a> {
    each: &'a [(Range<usize>, String)],
    next: usize,
    shift: isize,
}

impl Mapper<'_> {
    pub fn map(&mut self, at: usize) -> usize {
        while let Some((range, text)) = self.each.get(self.next) {
            let inserted = text.chars().count() as isize;
            if range.end <= at {
                self.shift += inserted - range.len() as isize;
                self.next += 1;
            } else if range.start < at {
                return (range.start as isize + self.shift + inserted) as usize;
            } else {
                break;
            }
        }
        (at as isize + self.shift) as usize
    }
}
