use std::ops::Range;

/// What a stretch of text is, for an editor to draw it by: what a parser says of it, or that a spellchecker found a mistake there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Keyword,
    Type,
    Literal,
    Comment,
    Mistake,
}

/// Stretches of a text and what each is, in characters, which an editor draws: a parser's marks in their inks, mistakes underlined. Work them out in a derived value, off the main thread, since they cover the whole text; the editor reads only the lines it shows. Marks a mistake may overlap; marks of the other kinds are kept in order and apart, and one that starts before the last one ends is left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    inks: Vec<(Range<usize>, Mark)>,
    mistakes: Vec<Range<usize>>,
}

impl Marks {
    pub fn new() -> Self {
        Marks::default()
    }

    /// Marks the characters in `range` as `mark`, after the marks of its kind added before it.
    pub fn add(&mut self, range: Range<usize>, mark: Mark) {
        if range.is_empty() {
            return;
        }
        match mark {
            Mark::Mistake if self.mistakes.last().is_none_or(|m| m.end <= range.start) => self.mistakes.push(range),
            Mark::Mistake => {}
            _ if self.inks.last().is_none_or(|(m, _)| m.end <= range.start) => self.inks.push((range, mark)),
            _ => {}
        }
    }

    /// The marks other than mistakes that reach into `range`, in order.
    pub fn inks_within(&self, range: Range<usize>) -> &[(Range<usize>, Mark)] {
        let from = self.inks.partition_point(|(m, _)| m.end <= range.start);
        let to = self.inks.partition_point(|(m, _)| m.start < range.end);
        &self.inks[from..to.max(from)]
    }

    /// The mistakes that reach into `range`, in order.
    pub fn mistakes_within(&self, range: Range<usize>) -> &[Range<usize>] {
        let from = self.mistakes.partition_point(|m| m.end <= range.start);
        let to = self.mistakes.partition_point(|m| m.start < range.end);
        &self.mistakes[from..to.max(from)]
    }

    /// Every mistake, in order.
    pub fn mistakes(&self) -> &[Range<usize>] {
        &self.mistakes
    }

    pub fn is_empty(&self) -> bool {
        self.inks.is_empty() && self.mistakes.is_empty()
    }
}
