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
        let mut shift = 0isize;
        for (range, text) in &self.each {
            let inserted = text.chars().count() as isize;
            if range.end <= at {
                shift += inserted - range.len() as isize;
            } else if range.start < at {
                return (range.start as isize + shift + inserted) as usize;
            } else {
                break;
            }
        }
        (at as isize + shift) as usize
    }
}
