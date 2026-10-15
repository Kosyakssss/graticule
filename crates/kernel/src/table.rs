//! Tables: rows of a list in named columns, sorted by a fact and in an order worked out in the background.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

use ledger::{Field, Key, List, Record, Value};

use crate::live::Derived;
use crate::tree::Tone;

/// Which column a table is sorted by and which way, kept in the facts: `#[session] sort: Sort`, or a document field when the order belongs to the document. Pressing a column's head sorts by it, then the other way, then not at all.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sort {
    column: String,
    descending: bool,
}

impl Sort {
    /// Sorted by the column named `column`, smallest first.
    pub fn by(column: impl Into<String>) -> Sort {
        Sort { column: column.into(), descending: false }
    }

    /// The same column, the other way round.
    pub fn reversed(self) -> Sort {
        Sort { descending: !self.descending, ..self }
    }

    /// The name of the column it is sorted by, as its head says it, or `""` when it isn't sorted.
    pub fn column(&self) -> &str {
        &self.column
    }

    pub fn descending(&self) -> bool {
        self.descending
    }

    pub fn is_sorted(&self) -> bool {
        !self.column.is_empty()
    }

    /// What pressing the head of `column` makes of it.
    pub(crate) fn pressed(&self, column: &str) -> Sort {
        match (self.column == column, self.descending) {
            (false, _) => Sort::by(column),
            (true, false) => self.clone().reversed(),
            (true, true) => Sort::default(),
        }
    }

    /// How the change reads in undo and the log.
    pub(crate) fn said(&self) -> String {
        match (self.is_sorted(), self.descending) {
            (false, _) => "Not sorted".to_string(),
            (true, false) => format!("Sorted by {}", self.column),
            (true, true) => format!("Sorted by {}, largest first", self.column),
        }
    }
}

impl Field for Sort {
    ledger::register!(Sort);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::List(vec![Value::Str(self.column.clone()), Value::Bool(self.descending)])
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::List(parts) => match parts.as_slice() {
                [Value::Str(column), Value::Bool(descending)] => Some(Sort { column: column.clone(), descending: *descending }),
                _ => None,
            },
            _ => None,
        }
    }
}
impl ledger::Document for Sort {}

/// The order a table shows a list's items in, worked out in the background as a derived value: `#[derived(File::order, from = [files, sort])] order: Derived<Order<File>>`. It may leave items out, which is how a table is filtered.
pub struct Order<T> {
    ranks: Arc<Ranks>,
    of: PhantomData<fn() -> T>,
}

/// The keys of an order's items, top to bottom, and where each one is.
#[derive(Debug, Default)]
pub struct Ranks {
    keys: Vec<u128>,
    at: HashMap<u128, u32>,
}

impl Ranks {
    fn new(keys: Vec<u128>) -> Ranks {
        let at = keys.iter().enumerate().map(|(i, &k)| (k, i as u32)).collect();
        Ranks { keys, at }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn key(&self, index: usize) -> Option<u128> {
        self.keys.get(index).copied()
    }

    pub fn position(&self, key: u128) -> Option<usize> {
        self.at.get(&key).map(|&i| i as usize)
    }
}

impl<T: Record> Order<T> {
    /// The list's own order.
    pub fn of(list: &List<T>) -> Order<T> {
        Order::from_keys(list.keys().map(Key::raw).collect())
    }

    /// The list's items sorted as `sort` says by `key`, which gives what each item is sorted by in that column. Items that are the same by it keep their order.
    pub fn by<K: Ord>(list: &List<T>, sort: &Sort, key: impl Fn(&T) -> K) -> Order<T> {
        let mut keyed: Vec<(K, u128)> = list.iter().map(|t| (key(t), t.key().raw())).collect();
        if sort.descending {
            keyed.sort_by(|a, b| b.0.cmp(&a.0));
        } else {
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
        }
        Order::from_keys(keyed.into_iter().map(|(_, k)| k).collect())
    }

    /// Only the items `keep` says yes to, in this order.
    pub fn only(self, list: &List<T>, keep: impl Fn(&T) -> bool) -> Order<T> {
        let keys = self.ranks.keys.iter().copied().filter(|&k| list.find(Key::from_raw(k)).is_some_and(&keep)).collect();
        Order::from_keys(keys)
    }

    fn from_keys(keys: Vec<u128>) -> Order<T> {
        Order { ranks: Arc::new(Ranks::new(keys)), of: PhantomData }
    }

    pub fn len(&self) -> usize {
        self.ranks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranks.is_empty()
    }
}

impl<T> Clone for Order<T> {
    fn clone(&self) -> Self {
        Order { ranks: self.ranks.clone(), of: PhantomData }
    }
}

impl<T: Record> Derived<Order<T>> {
    /// The rows of `list` in this order, once it has been worked out, and in the list's own order until then.
    pub fn of<'a>(&'a self, list: &'a List<T>) -> Rows<'a, T> {
        Rows { list, ranks: self.get().map(|o| &o.ranks) }
    }
}

/// The rows a list or a table shows: a list's items in its own order, or in an [`Order`].
pub struct Rows<'a, T: Record> {
    list: &'a List<T>,
    ranks: Option<&'a Arc<Ranks>>,
}

impl<'a, T: Record> From<&'a List<T>> for Rows<'a, T> {
    fn from(list: &'a List<T>) -> Self {
        Rows { list, ranks: None }
    }
}

impl<'a, T: Record> Rows<'a, T> {
    pub(crate) fn list(&self) -> &'a List<T> {
        self.list
    }

    pub(crate) fn ranks(&self) -> Option<&'a Arc<Ranks>> {
        self.ranks
    }

    pub fn len(&self) -> usize {
        self.ranks.map_or(self.list.len(), |r| r.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<&'a T> {
        match self.ranks {
            Some(r) => self.list.find(Key::from_raw(r.key(index)?)),
            None => self.list.get(index),
        }
    }

    pub fn position(&self, key: Key<T>) -> Option<usize> {
        match self.ranks {
            Some(r) => r.position(key.raw()),
            None => self.list.position(key),
        }
    }

    pub fn first(&self) -> Option<&'a T> {
        self.get(0)
    }

    pub fn last(&self) -> Option<&'a T> {
        self.get(self.len().checked_sub(1)?)
    }
}

/// What a table's cell shows: words, which read from the left, or a number, which lines up on the right in the monospace face.
pub trait Cell {
    fn shown(&self) -> String;

    fn numeric(&self) -> bool {
        false
    }

    /// Whether it is below zero, zero or above it, for cells toned by their sign.
    fn sign(&self) -> i8 {
        0
    }
}

impl Cell for str {
    fn shown(&self) -> String {
        self.to_string()
    }
}

impl Cell for String {
    fn shown(&self) -> String {
        self.clone()
    }
}

impl Cell for ledger::Text {
    fn shown(&self) -> String {
        self.as_str().to_string()
    }
}

impl<T: Cell + ?Sized> Cell for &T {
    fn shown(&self) -> String {
        (**self).shown()
    }

    fn numeric(&self) -> bool {
        (**self).numeric()
    }

    fn sign(&self) -> i8 {
        (**self).sign()
    }
}

macro_rules! whole {
    ($($t:ty),*) => {$(
        impl Cell for $t {
            fn shown(&self) -> String {
                crate::value::grouped(*self)
            }

            fn numeric(&self) -> bool {
                true
            }

            fn sign(&self) -> i8 {
                crate::value::Integer::wide(*self).signum() as i8
            }
        }
    )*};
}

whole!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, i128);

impl Cell for crate::days::Day {
    fn shown(&self) -> String {
        self.to_string()
    }

    fn numeric(&self) -> bool {
        true
    }
}

impl Cell for crate::days::Time {
    fn shown(&self) -> String {
        self.to_string()
    }

    fn numeric(&self) -> bool {
        true
    }
}

/// How a column's cells are toned.
pub(crate) fn toned(sign: i8, by_sign: bool) -> Tone {
    match (by_sign, sign.signum()) {
        (true, 1) => Tone::Ok,
        (true, -1) => Tone::Danger,
        _ => Tone::Ink,
    }
}
