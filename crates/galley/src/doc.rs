use std::fmt;
use std::ops::Range;

use ledger::{Change, Editable, ErasedPath, Field, Op, Outcome, Step, Travel, Typed, Value};
use ropey::Rope;

use crate::changes::Changes;
use crate::motion::{self, Motion};
use crate::selection::{Selections, Span};

/// Text of any length with its cursors, kept in a rope so an edit anywhere costs the same in a line or in a hundred megabytes. It is a field of the facts: each edit is recorded as an operation, so undo reverses it, and the cursors move with the text and come back with undo. Positions are counted in characters.
#[derive(Clone, Default)]
pub struct Doc {
    rope: Rope,
    selections: Selections,
    at: ErasedPath,
    revision: u64,
}

impl Doc {
    pub fn new(text: &str) -> Self {
        Doc { rope: Rope::from_str(text), selections: Selections::default(), at: ErasedPath::ROOT, revision: 0 }
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

    /// The cursors and selections.
    pub fn selections(&self) -> &Selections {
        &self.selections
    }

    /// Puts the cursors and selections somewhere else.
    pub fn select(&mut self, selections: Selections) {
        self.selections = Selections::new(selections.spans().to_vec(), selections.primary_index(), self.len());
    }

    /// Replaces a range of characters with new text, carrying the cursors with it. A range past the end changes nothing.
    pub fn replace(&mut self, range: Range<usize>, inserted: &str) -> Outcome {
        self.change(&Changes::new([(range, inserted.to_string())]))
    }

    /// Makes all the changes at once and moves the cursors with the text under them.
    pub fn change(&mut self, changes: &Changes) -> Outcome {
        let mut outcome = Outcome::Nothing;
        for (range, text) in changes.iter().rev() {
            if self.splice(range.clone(), text).changed() {
                outcome = Outcome::Changed;
            }
        }
        let len = self.len();
        self.selections.map(changes, len);
        outcome
    }

    fn splice(&mut self, range: Range<usize>, inserted: &str) -> Outcome {
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
        self.revision += 1;
        Outcome::Changed
    }

    /// Types text at every cursor, replacing what each selects, and leaves each cursor after it.
    pub fn insert(&mut self, text: &str) -> Outcome {
        let changes = Changes::new(self.selections.spans().iter().map(|s| (s.range(), text.to_string())));
        self.typed(&changes, |s| s.range().end)
    }

    /// Starts a new line at every cursor, indented like the line it was on.
    pub fn insert_line(&mut self) -> Outcome {
        let changes = Changes::new(self.selections.spans().iter().map(|s| {
            let line = self.line(self.line_of(s.range().start));
            let indent: String = self.text(line).chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            (s.range(), format!("\n{indent}"))
        }));
        self.typed(&changes, |s| s.range().end)
    }

    /// Deletes what each selection holds, or at each cursor the character or word before it, or after it when `forward`.
    pub fn delete(&mut self, forward: bool, word: bool) -> Outcome {
        let changes = Changes::new(self.selections.spans().iter().map(|s| {
            let range = match (s.is_empty(), forward, word) {
                (false, ..) => s.range(),
                (true, false, false) => motion::before(self, s.head)..s.head,
                (true, true, false) => s.head..motion::after(self, s.head),
                (true, false, true) => self.motion(s.head, Motion::WordPrevious)..s.head,
                (true, true, true) => s.head..self.motion(s.head, Motion::WordEnd),
            };
            (range, String::new())
        }));
        self.typed(&changes, |s| s.range().start)
    }

    fn typed(&mut self, changes: &Changes, rest: impl Fn(Span) -> usize) -> Outcome {
        let outcome = self.change(changes);
        let len = self.len();
        self.selections.each(len, |s| Span::cursor(rest(s)));
        outcome
    }

    /// Moves every cursor by `motion`. Extending keeps where each began, so it selects what was passed over; otherwise a selection moved left or right becomes a cursor at its start or end.
    pub fn go(&mut self, motion: Motion, extend: bool) {
        let doc = &*self;
        let mut selections = self.selections.clone();
        selections.each(doc.len(), |s| match (extend, motion) {
            (false, Motion::Left) if !s.is_empty() => Span::cursor(s.range().start),
            (false, Motion::Right) if !s.is_empty() => Span::cursor(s.range().end),
            (true, _) => Span::new(s.anchor, doc.motion(s.head, motion)),
            (false, _) => Span::cursor(doc.motion(s.head, motion)),
        });
        self.selections = selections;
    }

    /// Puts a single cursor at `at`, or extends the primary selection to it.
    pub fn place(&mut self, at: usize, extend: bool) {
        let at = at.min(self.len());
        let anchor = if extend { self.selections.primary().anchor } else { at };
        self.selections = Selections::new(vec![Span::new(anchor, at)], 0, self.len());
    }

    /// Adds a cursor at `at` and makes it primary.
    pub fn add_cursor(&mut self, at: usize) {
        let len = self.len();
        self.selections.add(Span::cursor(at), len);
    }

    pub fn select_all(&mut self) {
        self.selections = Selections::new(vec![Span::new(0, self.len())], 0, self.len());
    }

    /// Keeps only the primary cursor or selection, and when that is all there is, lets go of what it selects.
    pub fn collapse(&mut self) {
        if self.selections.spans().len() > 1 {
            self.selections.only_primary();
        } else {
            let head = self.selections.primary().head;
            self.place(head, false);
        }
    }

    /// Answers what was typed into an editor showing it.
    pub fn typed_in(&mut self, typed: &Typed) -> Outcome {
        let before = self.selections.clone();
        let outcome = match typed {
            Typed::Insert(text) => return self.insert(text),
            Typed::NewLine => return self.insert_line(),
            Typed::Delete { forward, word } => return self.delete(*forward, *word),
            Typed::Move { to, extend } => {
                let (motion, times) = match to {
                    Travel::Left => (Motion::Left, 1),
                    Travel::Right => (Motion::Right, 1),
                    Travel::WordLeft => (Motion::WordPrevious, 1),
                    Travel::WordRight => (Motion::WordEnd, 1),
                    Travel::Up => (Motion::Up, 1),
                    Travel::Down => (Motion::Down, 1),
                    Travel::LineStart => (Motion::LineStart, 1),
                    Travel::LineEnd => (Motion::LineEnd, 1),
                    Travel::PageUp(n) => (Motion::Up, (*n).max(1)),
                    Travel::PageDown(n) => (Motion::Down, (*n).max(1)),
                    Travel::Start => (Motion::Start, 1),
                    Travel::End => (Motion::End, 1),
                };
                for _ in 0..times {
                    self.go(motion, *extend);
                }
                Outcome::Nothing
            }
            Typed::Place { at, extend } => {
                self.place(*at, *extend);
                Outcome::Nothing
            }
            Typed::SelectAll => {
                self.select_all();
                Outcome::Nothing
            }
            Typed::Collapse => {
                self.collapse();
                Outcome::Nothing
            }
        };
        if self.selections != before { Outcome::Changed } else { outcome }
    }

    pub(crate) fn rope(&self) -> &Rope {
        &self.rope
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn swap(&mut self, rope: Rope, selections: Selections) {
        self.rope = rope;
        self.selections = selections;
        self.revision += 1;
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
    type Snap = Selections;

    fn snap(&self) -> Selections {
        self.selections.clone()
    }

    fn diff(&self, before: &Selections, at: ErasedPath, out: &mut Vec<Op>) {
        if self.at != at {
            ledger::replaced(std::any::type_name::<Self>());
        }
        if self.selections != *before {
            out.push(Op::Select { selection: at, before: before.value(), after: self.selections.value() });
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
                self.revision += 1;
                true
            }
            ([], Op::Select { after, .. }) => match Selections::from_value(after) {
                Some(selections) => {
                    self.selections = selections;
                    true
                }
                None => false,
            },
            ([], Op::Set { after: Value::Str(s), .. }) => {
                self.rope = Rope::from_str(s);
                self.revision += 1;
                let len = self.len();
                self.selections.each(len, |s| s);
                true
            }
            _ => false,
        }
    }

    fn edit(&mut self, steps: &[Step], change: &Change) -> Outcome {
        match (steps, change) {
            ([], Change::Typed(typed)) => self.typed_in(typed),
            ([], Change::Set(Value::Str(s))) => {
                let len = self.len();
                self.replace(0..len, s)
            }
            _ => Outcome::Nothing,
        }
    }
}

impl ledger::Document for Doc {}

impl Editable for Doc {
    fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    fn line(&self, line: usize, each: &mut dyn FnMut(&str)) {
        for chunk in self.rope.slice(Doc::line(self, line)).chunks() {
            each(chunk);
        }
    }

    fn line_start(&self, line: usize) -> usize {
        Doc::line(self, line).start
    }

    fn line_of(&self, at: usize) -> usize {
        Doc::line_of(self, at)
    }

    fn cursors(&self, out: &mut Vec<(usize, usize)>) -> usize {
        let len = self.len();
        out.extend(self.selections.spans().iter().map(|s| (s.anchor.min(len), s.head.min(len))));
        self.selections.primary_index()
    }

    fn selected(&self) -> String {
        let parts: Vec<String> = self.selections.spans().iter().filter(|s| !s.is_empty()).map(|s| self.text(s.range())).collect();
        parts.join("\n")
    }
}
