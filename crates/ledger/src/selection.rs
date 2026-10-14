use std::collections::BTreeSet;
use std::fmt;

use crate::journal;
use crate::key::Key;
use crate::op::Op;
use crate::outcome::Outcome;
use crate::path::{ErasedPath, Field, Step};
use crate::value::Value;

/// Which items of a list are chosen, and which one has focus. It is session state, and keys of items that no longer exist are dropped whenever the document changes.
pub struct Selection<T> {
    keys: BTreeSet<Key<T>>,
    focus: Option<Key<T>>,
    anchor: Option<Key<T>>,
    at: ErasedPath,
}

impl<T> Default for Selection<T> {
    fn default() -> Self {
        Self { keys: BTreeSet::new(), focus: None, anchor: None, at: ErasedPath::ROOT }
    }
}

impl<T> Clone for Selection<T> {
    fn clone(&self) -> Self {
        Self { keys: self.keys.clone(), focus: self.focus, anchor: self.anchor, at: self.at }
    }
}

impl<T> PartialEq for Selection<T> {
    fn eq(&self, other: &Self) -> bool {
        self.keys == other.keys && self.focus == other.focus && self.anchor == other.anchor
    }
}

impl<T> fmt::Debug for Selection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Selection").field("keys", &self.keys).field("focus", &self.focus).finish()
    }
}

/// A change to a selection, as the kernel asks for it when a list is clicked or moved through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choose {
    Only(u128),
    Toggle(u128),
    Extend { to: u128, between: Vec<u128> },
    Focus(u128),
    All(Vec<u128>),
    Clear,
}

impl<T: 'static> Selection<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn focused(&self) -> Option<Key<T>> {
        self.focus
    }

    pub fn anchor(&self) -> Option<Key<T>> {
        self.anchor.or(self.focus)
    }

    pub fn contains(&self, key: Key<T>) -> bool {
        self.keys.contains(&key)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = Key<T>> + '_ {
        self.keys.iter().copied()
    }

    pub fn path(&self) -> ErasedPath {
        self.at
    }

    /// Changes the selection. Changing only the selection is never an undo step, but every step remembers the selection from before it.
    pub fn choose(&mut self, choice: &Choose) -> Outcome {
        let before = self.value();
        match choice {
            Choose::Only(k) => {
                let k = Key::from_raw(*k);
                self.keys.clear();
                self.keys.insert(k);
                self.focus = Some(k);
                self.anchor = Some(k);
            }
            Choose::Toggle(k) => {
                let k = Key::from_raw(*k);
                if !self.keys.remove(&k) {
                    self.keys.insert(k);
                }
                self.focus = Some(k);
                self.anchor = Some(k);
            }
            Choose::Extend { to, between } => {
                self.keys.clear();
                self.keys.extend(between.iter().map(|k| Key::from_raw(*k)));
                self.focus = Some(Key::from_raw(*to));
            }
            Choose::Focus(k) => {
                let k = Key::from_raw(*k);
                self.focus = Some(k);
                self.anchor = Some(k);
                self.keys.clear();
                self.keys.insert(k);
            }
            Choose::All(keys) => {
                self.keys = keys.iter().map(|k| Key::from_raw(*k)).collect();
                if self.focus.is_none() {
                    self.focus = self.keys.first().copied();
                }
            }
            Choose::Clear => {
                self.keys.clear();
                self.anchor = self.focus;
            }
        }
        let after = self.value();
        if after == before {
            return Outcome::Nothing;
        }
        journal::record(|| Op::Select { selection: self.at, before, after });
        Outcome::Changed
    }

    fn restore(&mut self, value: &Value) -> bool {
        let Value::List(parts) = value else { return false };
        let [Value::List(keys), focus, anchor] = parts.as_slice() else { return false };
        let key = |v: &Value| match v {
            Value::Key(k) => Some(Key::from_raw(*k)),
            _ => None,
        };
        self.keys = keys.iter().filter_map(key).collect();
        self.focus = key(focus);
        self.anchor = key(anchor);
        true
    }
}

impl<T: 'static> Field for Selection<T> {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), _: ErasedPath, _: &mut Vec<Op>) {}

    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn value(&self) -> Value {
        let key = |k: Option<Key<T>>| k.map_or(Value::None, |k| Value::Key(k.raw()));
        Value::List(vec![Value::List(self.keys.iter().map(|k| Value::Key(k.raw())).collect()), key(self.focus), key(self.anchor)])
    }

    fn from_value(value: &Value) -> Option<Self> {
        let mut s = Selection::default();
        s.restore(value).then_some(s)
    }

    fn attach(&mut self, at: ErasedPath) {
        self.at = at;
    }

    fn apply(&mut self, steps: &[Step], op: &Op) -> bool {
        match (steps, op) {
            ([], Op::Select { after, .. }) | ([], Op::Set { after, .. }) => self.restore(after),
            _ => false,
        }
    }

    fn edit(&mut self, steps: &[Step], change: &crate::Change) -> Outcome {
        match (steps, change) {
            ([], crate::Change::Select(choice)) => self.choose(choice),
            _ => Outcome::Nothing,
        }
    }

    fn prune(&mut self, deleted: &[u128]) {
        if deleted.is_empty() {
            return;
        }
        for k in deleted {
            self.keys.remove(&Key::from_raw(*k));
        }
        if self.focus.is_some_and(|f| deleted.contains(&f.raw())) {
            self.focus = self.keys.first().copied();
        }
        if self.anchor.is_some_and(|a| deleted.contains(&a.raw())) {
            self.anchor = self.focus;
        }
    }
}
