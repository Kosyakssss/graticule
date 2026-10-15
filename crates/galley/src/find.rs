use std::ops::Range;

use ledger::{Field, Op, Outcome, Value};
use ropey::Rope;

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
    pub fn replace_all(&mut self, needle: &str, with: &str) -> (usize, Outcome) {
        let found = self.find_all(needle);
        let changes = Changes::new(found.iter().map(|r| (r.clone(), with.to_string())));
        (found.len(), self.change(&changes))
    }

    /// Works out, on a copy, what replacing every place `needle` appears with `with` would make, so a long text can be searched and rebuilt in a job and the result taken in with [`Doc::take`] at no more cost than a short one. Cloning a `Doc` to send into the job is cheap: the copy shares the text's storage.
    pub fn replacement(&self, needle: &str, with: &str) -> Replacement {
        let found = self.find_all(needle);
        let (Some(first), Some(last)) = (found.first(), found.last()) else {
            return Replacement { count: 0, start: 0, end: 0, inserted: String::new(), selections: self.selections().clone(), ready: None };
        };
        let (start, end) = (first.start, last.end);
        let mut inserted = String::with_capacity(self.rope().slice(start..end).len_bytes());
        let mut at = start;
        for range in &found {
            inserted.extend(self.rope().slice(at..range.start).chunks());
            inserted.push_str(with);
            at = range.end;
        }
        let removed = self.rope().slice(start..end).to_string();
        let mut rope = self.rope().clone();
        rope.remove(start..end);
        rope.insert(start, &inserted);
        let mut selections = self.selections().clone();
        selections.map(&Changes::new(found.iter().map(|r| (r.clone(), with.to_string()))), rope.len_chars());
        let byte = self.rope().char_to_byte(start);
        Replacement { count: found.len(), start, end, inserted, selections, ready: Some(Ready { revision: self.revision(), rope, removed, byte }) }
    }

    /// Takes in a replacement worked out by [`Doc::replacement`]: one change, undone in one step. It is refused when the text changed after it was worked out, since its places no longer mean what they meant.
    pub fn take(&mut self, replacement: Replacement) -> Result<Outcome, Stale> {
        let Replacement { count, start, end, inserted, selections, ready } = replacement;
        if count == 0 {
            return Ok(Outcome::Nothing);
        }
        match ready {
            Some(ready) if ready.revision != self.revision() => Err(Stale),
            Some(Ready { rope, removed, byte, .. }) => {
                let at = self.path();
                ledger::record(|| Op::Splice { text: at, at: byte, removed, inserted });
                self.swap(rope, selections);
                Ok(Outcome::Changed)
            }
            None if end <= self.len() => {
                let outcome = self.replace(start..end, &inserted);
                self.select(selections);
                Ok(outcome)
            }
            None => Err(Stale),
        }
    }

    /// Selects every place `needle` appears and says how many there were, keeping the selections as they are when it appears nowhere.
    pub fn select_matches(&mut self, needle: &str) -> usize {
        let found = self.find_all(needle);
        let count = found.len();
        if count > 0 {
            self.select(Selections::new(found.into_iter().map(|r| Span::new(r.start, r.end)).collect(), 0, self.len()));
        }
        count
    }
}

/// Every place a word appears replaced, worked out on a copy of a [`Doc`] by [`Doc::replacement`] and taken in by [`Doc::take`]. It travels as a command's argument, so the command log keeps the change it makes; a replay recomputes the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    count: usize,
    start: usize,
    end: usize,
    inserted: String,
    selections: Selections,
    ready: Option<Ready>,
}

impl Replacement {
    /// How many places were replaced.
    pub fn count(&self) -> usize {
        self.count
    }
}

#[derive(Clone, Debug)]
struct Ready {
    revision: u64,
    rope: Rope,
    removed: String,
    byte: usize,
}

impl PartialEq for Ready {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision && self.byte == other.byte && self.removed == other.removed
    }
}

impl Eq for Ready {}

/// A replacement refused because the text changed after it was worked out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stale;

impl Field for Replacement {
    ledger::register!(Replacement);

    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn value(&self) -> Value {
        Value::Map(vec![
            ("count".into(), Value::UInt(self.count as u64)),
            ("start".into(), Value::UInt(self.start as u64)),
            ("end".into(), Value::UInt(self.end as u64)),
            ("inserted".into(), Value::Str(self.inserted.clone())),
            ("selections".into(), self.selections.value()),
        ])
    }

    fn from_value(value: &Value) -> Option<Self> {
        let Value::Map(fields) = value else { return None };
        let get = |name: &str| fields.iter().find(|(k, _)| k == name).map(|(_, v)| v);
        let number = |name: &str| match get(name) {
            Some(Value::UInt(n)) => Some(*n as usize),
            _ => None,
        };
        let inserted = match get("inserted") {
            Some(Value::Str(s)) => s.clone(),
            _ => return None,
        };
        Some(Replacement { count: number("count")?, start: number("start")?, end: number("end")?, inserted, selections: Selections::from_value(get("selections")?)?, ready: None })
    }
}

impl ledger::Document for Replacement {}

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
