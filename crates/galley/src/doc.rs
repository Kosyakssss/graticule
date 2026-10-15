use std::fmt;
use std::ops::Range;

use ledger::{ErasedPath, Field, Op, Outcome, Step, Value};
use ropey::Rope;

use crate::changes::Changes;
use crate::motion;
use crate::selection::{Selections, Span};

/// Text of any length, kept in a rope so an edit anywhere costs the same in a line or in a hundred megabytes. It is a field of the facts: each edit is recorded as an operation, so undo reverses it and saving writes it.
#[derive(Clone, Default)]
pub struct Doc {
    rope: Rope,
    at: ErasedPath,
}

impl Doc {
    pub fn new(text: &str) -> Self {
        Doc { rope: Rope::from_str(text), at: ErasedPath::ROOT }
    }

    /// How many characters it holds.
    pub fn len(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// How many lines it has: one more than its line breaks.
    pub fn lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// The line the character at `at` is on.
    pub fn line_of(&self, at: usize) -> usize {
        self.rope.char_to_line(at.min(self.len()))
    }

    /// The characters of a line, without its line break.
    pub fn line(&self, line: usize) -> Range<usize> {
        if line >= self.lines() {
            return self.len()..self.len();
        }
        let start = self.rope.line_to_char(line);
        let mut end = if line + 1 < self.lines() { self.rope.line_to_char(line + 1) } else { self.len() };
        while end > start && matches!(self.rope.char(end - 1), '\n' | '\r') {
            end -= 1;
        }
        start..end
    }

    /// The character at `at`, if there is one.
    pub fn char(&self, at: usize) -> Option<char> {
        (at < self.len()).then(|| self.rope.char(at))
    }

    /// The text in a range of characters.
    pub fn text(&self, range: Range<usize>) -> String {
        let end = range.end.min(self.len());
        self.rope.slice(range.start.min(end)..end).to_string()
    }

    /// Replaces a range of characters with new text. A range past the end changes nothing.
    pub fn replace(&mut self, range: Range<usize>, inserted: &str) -> Outcome {
        if range.start > range.end || range.end > self.len() || (range.is_empty() && inserted.is_empty()) || self.rope.slice(range.clone()) == inserted {
            return Outcome::Nothing;
        }
        if ledger::recording() {
            let removed = self.rope.slice(range.clone()).to_string();
            let at = self.rope.char_to_byte(range.start);
            ledger::record(|| Op::Splice { text: self.at, at, removed, inserted: inserted.to_string() });
        }
        self.rope.remove(range.clone());
        self.rope.insert(range.start, inserted);
        Outcome::Changed
    }

    /// Makes all the changes at once and moves the selections with the text under them.
    pub fn change(&mut self, changes: &Changes, selections: &mut Selections) -> Outcome {
        let mut outcome = Outcome::Nothing;
        for (range, text) in changes.iter().rev() {
            if self.replace(range.clone(), text).changed() {
                outcome = Outcome::Changed;
            }
        }
        selections.map(changes, self.len());
        outcome
    }

    /// Types text at every cursor, replacing what each selects, and leaves each cursor after it.
    pub fn insert(&mut self, selections: &mut Selections, text: &str) -> Outcome {
        let changes = Changes::new(selections.spans().iter().map(|s| (s.range(), text.to_string())));
        let outcome = self.change(&changes, selections);
        selections.each(self.len(), |s| Span::cursor(s.head.max(s.anchor)));
        outcome
    }

    /// Starts a new line at every cursor, indented like the line it was on.
    pub fn insert_line(&mut self, selections: &mut Selections) -> Outcome {
        let changes = Changes::new(selections.spans().iter().map(|s| {
            let line = self.line(self.line_of(s.range().start));
            let indent: String = self.text(line).chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            (s.range(), format!("\n{indent}"))
        }));
        let outcome = self.change(&changes, selections);
        selections.each(self.len(), |s| Span::cursor(s.head.max(s.anchor)));
        outcome
    }

    /// Deletes what each selection holds, or the character before each cursor.
    pub fn delete_back(&mut self, selections: &mut Selections) -> Outcome {
        let changes = Changes::new(selections.spans().iter().map(|s| match s.is_empty() {
            true => (motion::before(self, s.head)..s.head, String::new()),
            false => (s.range(), String::new()),
        }));
        let outcome = self.change(&changes, selections);
        selections.each(self.len(), |s| Span::cursor(s.head.min(s.anchor)));
        outcome
    }

    /// Deletes what each selection holds, or the character after each cursor.
    pub fn delete_forward(&mut self, selections: &mut Selections) -> Outcome {
        let changes = Changes::new(selections.spans().iter().map(|s| match s.is_empty() {
            true => (s.head..motion::after(self, s.head), String::new()),
            false => (s.range(), String::new()),
        }));
        let outcome = self.change(&changes, selections);
        selections.each(self.len(), |s| Span::cursor(s.head.min(s.anchor)));
        outcome
    }

    pub(crate) fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn path(&self) -> ErasedPath {
        self.at
    }
}

impl PartialEq for Doc {
    fn eq(&self, other: &Self) -> bool {
        self.rope == other.rope
    }
}

impl Eq for Doc {}

impl PartialEq<&str> for Doc {
    fn eq(&self, other: &&str) -> bool {
        self.rope == *other
    }
}

impl fmt::Debug for Doc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.rope.to_string(), f)
    }
}

impl fmt::Display for Doc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.rope.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}

impl From<&str> for Doc {
    fn from(s: &str) -> Self {
        Doc::new(s)
    }
}

impl Field for Doc {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), at: ErasedPath, _: &mut Vec<Op>) {
        if self.at != at {
            ledger::replaced(std::any::type_name::<Self>());
        }
    }

    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn value(&self) -> Value {
        Value::Str(self.to_string())
    }

    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Str(s) => Some(Doc::new(s)),
            _ => None,
        }
    }

    fn attach(&mut self, at: ErasedPath) {
        self.at = at;
    }

    fn apply(&mut self, steps: &[Step], op: &Op) -> bool {
        match (steps, op) {
            ([], Op::Splice { at, removed, inserted, .. }) => {
                let end = at + removed.len();
                if end > self.rope.len_bytes() {
                    return false;
                }
                let (start, stop) = (self.rope.byte_to_char(*at), self.rope.byte_to_char(end));
                if self.rope.char_to_byte(start) != *at || self.rope.char_to_byte(stop) != end || self.rope.slice(start..stop) != removed.as_str() {
                    return false;
                }
                self.rope.remove(start..stop);
                self.rope.insert(start, inserted);
                true
            }
            ([], Op::Set { after: Value::Str(s), .. }) => {
                self.rope = Rope::from_str(s);
                true
            }
            _ => false,
        }
    }

    fn edit(&mut self, steps: &[Step], change: &ledger::Change) -> Outcome {
        match (steps, change) {
            ([], ledger::Change::Set(Value::Str(s))) => {
                let len = self.len();
                self.replace(0..len, s)
            }
            _ => Outcome::Nothing,
        }
    }
}

impl ledger::Document for Doc {}
