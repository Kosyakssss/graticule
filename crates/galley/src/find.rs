use std::ops::Range;

use ledger::Outcome;

use crate::changes::Changes;
use crate::doc::Doc;
use crate::selection::{Selections, Span};

impl Doc {
    /// Every place `needle` appears, in order and never overlapping, in characters. Its cost grows with the text, so on a long text it belongs in a job.
    pub fn find_all(&self, needle: &str) -> Vec<Range<usize>> {
        let needle = needle.as_bytes();
        if needle.is_empty() {
            return Vec::new();
        }
        let fallback = prefix_table(needle);
        let (mut found, mut matched) = (Vec::new(), 0);
        for (i, byte) in self.rope().bytes().enumerate() {
            while matched > 0 && needle[matched] != byte {
                matched = fallback[matched - 1];
            }
            if needle[matched] == byte {
                matched += 1;
            }
            if matched == needle.len() {
                let end = i + 1;
                let start = end - needle.len();
                found.push(self.rope().byte_to_char(start)..self.rope().byte_to_char(end));
                matched = 0;
            }
        }
        found
    }

    /// The first place `needle` appears at or after `from`, coming round from the start when it doesn't.
    pub fn find_next(&self, needle: &str, from: usize) -> Option<Range<usize>> {
        let all = self.find_all(needle);
        all.iter().find(|r| r.start >= from).or(all.first()).cloned()
    }

    /// Replaces every place `needle` appears with `with`, as one change, and says how many there were.
    pub fn replace_all(&mut self, needle: &str, with: &str, selections: &mut Selections) -> (usize, Outcome) {
        let found = self.find_all(needle);
        let changes = Changes::new(found.iter().map(|r| (r.clone(), with.to_string())));
        (found.len(), self.change(&changes, selections))
    }
}

impl Selections {
    /// Selects every place `needle` appears, keeping the current selection when it appears nowhere.
    pub fn select_all(&mut self, doc: &Doc, needle: &str) {
        let found = doc.find_all(needle);
        if !found.is_empty() {
            *self = Selections::new(found.into_iter().map(|r| Span::new(r.start, r.end)).collect(), 0, doc.len());
        }
    }
}

fn prefix_table(needle: &[u8]) -> Vec<usize> {
    let mut table = vec![0; needle.len()];
    let mut k = 0;
    for i in 1..needle.len() {
        while k > 0 && needle[i] != needle[k] {
            k = table[k - 1];
        }
        if needle[i] == needle[k] {
            k += 1;
        }
        table[i] = k;
    }
    table
}
