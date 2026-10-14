use std::fmt;
use std::ops::Range;

use crate::journal;
use crate::op::Op;
use crate::outcome::Outcome;
use crate::path::{ErasedPath, Field, Step};
use crate::plain::Document;
use crate::value::Value;

/// Text a field edits: a value changed by typing. The caret and the selected text belong to the field that shows it, so they are never saved or synced.
#[derive(Clone, Default)]
pub struct Text {
    text: String,
    at: ErasedPath,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), at: ErasedPath::ROOT }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Takes the text out, leaving the field empty: what adding an item from a field does.
    pub fn take(&mut self) -> String {
        if !self.text.is_empty() {
            let removed = self.text.clone();
            journal::record(|| Op::Splice { text: self.at, at: 0, removed, inserted: String::new() });
        }
        std::mem::take(&mut self.text)
    }

    pub fn set(&mut self, text: &str) -> Outcome {
        let len = self.text.len();
        self.splice(0..len, text)
    }

    /// Replaces a byte range with new text. Ranges outside the text or off a character boundary change nothing.
    pub fn splice(&mut self, range: Range<usize>, inserted: &str) -> Outcome {
        if range.start > range.end || range.end > self.text.len() || !self.text.is_char_boundary(range.start) || !self.text.is_char_boundary(range.end) {
            return Outcome::Nothing;
        }
        if range.is_empty() && inserted.is_empty() {
            return Outcome::Nothing;
        }
        if &self.text[range.clone()] == inserted {
            return Outcome::Nothing;
        }
        if journal::recording() {
            let removed = self.text[range.clone()].to_string();
            journal::record(|| Op::Splice { text: self.at, at: range.start, removed, inserted: inserted.to_string() });
        }
        self.text.replace_range(range, inserted);
        Outcome::Changed
    }

    pub fn path(&self) -> ErasedPath {
        self.at
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Text {}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.text, f)
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::new(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::new(s)
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl Field for Text {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), at: ErasedPath, _: &mut Vec<Op>) {
        if self.at != at {
            journal::replaced(std::any::type_name::<Self>());
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.text == other.text
    }

    fn value(&self) -> Value {
        Value::Str(self.text.clone())
    }

    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Str(s) => Some(Text::new(s.clone())),
            _ => None,
        }
    }

    fn edit(&mut self, steps: &[Step], change: &crate::Change) -> Outcome {
        match (steps, change) {
            ([], crate::Change::Splice { range, text }) => self.splice(range.clone(), text),
            ([], crate::Change::Set(Value::Str(s))) => self.set(s),
            _ => Outcome::Nothing,
        }
    }

    fn attach(&mut self, at: ErasedPath) {
        self.at = at;
    }

    fn apply(&mut self, steps: &[Step], op: &Op) -> bool {
        match (steps, op) {
            ([], Op::Splice { at, removed, inserted, .. }) => {
                let end = at + removed.len();
                if end > self.text.len() || !self.text.is_char_boundary(*at) || !self.text.is_char_boundary(end) || &self.text[*at..end] != removed {
                    return false;
                }
                self.text.replace_range(*at..end, inserted);
                true
            }
            ([], Op::Set { after: Value::Str(s), .. }) => {
                self.text = s.clone();
                true
            }
            _ => false,
        }
    }
}

impl Document for Text {}
