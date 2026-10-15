use std::ops::Range;

use ledger::{Mark, Marks};

use crate::doc::Doc;

/// A spellchecker, which Galley asks about words: the system's, or anything an app plugs in.
pub trait Spelling {
    /// Whether `word` is spelt as the language has it.
    fn knows(&self, word: &str) -> bool;

    /// Words that might have been meant instead of `word`, likeliest first.
    fn suggest(&self, word: &str) -> Vec<String>;
}

impl Doc {
    /// Marks each word in `range` that `spelling` doesn't know as a mistake, in order after what `marks` holds. A word here is letters with apostrophes inside; anything holding a digit or an underscore is a name, not a word, and is left alone. It reads the whole range, so run it in a derived value.
    pub fn check_spelling(&self, range: Range<usize>, spelling: &dyn Spelling, marks: &mut Marks) {
        let end = range.end.min(self.len());
        let mut at = range.start.min(end);
        let mut word = String::new();
        while at < end {
            let Some(found) = self.word_from(at, end, &mut word) else { break };
            if !spelling.knows(&word) {
                marks.add(found.clone(), Mark::Mistake);
            }
            at = found.end;
        }
    }

    /// The word the cursor at `at` is in or just after, if any.
    pub fn word_at(&self, at: usize) -> Option<Range<usize>> {
        let at = at.min(self.len());
        let is_word = |i: usize| self.char(i).is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '\'');
        let mut start = at;
        while start > 0 && is_word(start - 1) {
            start -= 1;
        }
        let mut end = at;
        while is_word(end) {
            end += 1;
        }
        (start < end).then_some(start..end)
    }

    fn word_from(&self, mut at: usize, end: usize, word: &mut String) -> Option<Range<usize>> {
        loop {
            while at < end && !self.char(at).is_some_and(|c| c.is_alphanumeric() || c == '_') {
                at += 1;
            }
            if at >= end {
                return None;
            }
            let start = at;
            word.clear();
            let mut name = false;
            while at < end {
                match self.char(at) {
                    Some(c) if c.is_alphabetic() => word.push(c),
                    Some('\'') if self.char(at + 1).is_some_and(char::is_alphabetic) && !word.is_empty() => word.push('\''),
                    Some(c) if c.is_alphanumeric() || c == '_' => {
                        name = true;
                        word.push(c);
                    }
                    _ => break,
                }
                at += 1;
            }
            if !name {
                return Some(start..at);
            }
        }
    }
}
