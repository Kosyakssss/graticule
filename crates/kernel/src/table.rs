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

/// The keys of an order's items, top to bottom, and where each one is; in a tree, also how deep each one sits.
#[derive(Debug, Default)]
pub struct Ranks {
    keys: Vec<u128>,
    at: HashMap<u128, u32>,
    nests: Vec<Nest>,
}

/// Whether a row of a tree holds others, and whether they show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Branch {
    #[default]
    Leaf,
    Closed,
    Open,
}

/// Where a row of a tree sits: how deep, whether it is a branch, and the place of the row it is inside, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nest {
    pub depth: u16,
    pub branch: Branch,
    pub parent: Option<u32>,
}

impl Ranks {
    fn new(keys: Vec<u128>) -> Ranks {
        let at = keys.iter().enumerate().map(|(i, &k)| (k, i as u32)).collect();
        Ranks { keys, at, nests: Vec::new() }
    }

    /// Where the row at `index` sits in its tree, when the order is a tree's.
    pub fn nest(&self, index: usize) -> Option<Nest> {
        self.nests.get(index).copied()
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

    /// The list's items sorted as `sort` says by the field its column is named for, as a table made by `ui.records` heads it. Numbers compare as numbers and words without regard to case; an item with nothing in the field comes last.
    pub fn sorted(list: &List<T>, sort: &Sort) -> Order<T> {
        let mut keyed: Vec<(Value, u128)> = list.iter().filter_map(|t| fields_of(t).into_iter().find(|(head, _)| head == sort.column()).map(|(_, v)| (v, t.key().raw()))).collect();
        if keyed.len() != list.len() {
            return Order::of(list);
        }
        keyed.sort_by(|a, b| {
            let order = compare(&a.0, &b.0);
            if sort.descending && !matches!((&a.0, &b.0), (Value::None, _) | (_, Value::None)) { order.reverse() } else { order }
        });
        Order::from_keys(keyed.into_iter().map(|(_, k)| k).collect())
    }

    /// The rows of a tree: each item under the one `parent` names, the items with none at the top, and the items inside a branch shown only while `open` holds its key. Items keep the list's order among their siblings.
    pub fn tree(list: &List<T>, open: &[Key<T>], parent: impl Fn(&T) -> Option<Key<T>>) -> Order<T> {
        let mut under: HashMap<u128, Vec<u128>> = HashMap::new();
        let mut roots = Vec::new();
        for t in list.iter() {
            match parent(t).filter(|p| list.has(*p)) {
                Some(p) => under.entry(p.raw()).or_default().push(t.key().raw()),
                None => roots.push(t.key().raw()),
            }
        }
        let open: std::collections::HashSet<u128> = open.iter().map(|k| k.raw()).collect();
        let (mut keys, mut nests) = (Vec::with_capacity(list.len()), Vec::with_capacity(list.len()));
        let mut stack: Vec<(u128, u16, Option<u32>)> = roots.into_iter().rev().map(|k| (k, 0, None)).collect();
        while let Some((key, depth, up)) = stack.pop() {
            let at = keys.len() as u32;
            let inside = under.get(&key);
            let branch = match (inside, open.contains(&key)) {
                (None, _) => Branch::Leaf,
                (Some(_), false) => Branch::Closed,
                (Some(_), true) => Branch::Open,
            };
            keys.push(key);
            nests.push(Nest { depth, branch, parent: up });
            if branch == Branch::Open
                && let Some(inside) = inside
            {
                stack.extend(inside.iter().rev().map(|&k| (k, depth + 1, Some(at))));
            }
        }
        let mut ranks = Ranks::new(keys);
        ranks.nests = nests;
        Order { ranks: Arc::new(ranks), of: PhantomData }
    }

    /// The items whose words match `query`, best first, as a picker shows them: the letters typed appear in order in what `words` gives, and matches where they start words or sit together come first. Everything, in the list's order, while the query is empty.
    pub fn found<'t>(list: &'t List<T>, query: &str, words: impl Fn(&'t T) -> &'t str) -> Order<T> {
        if query.trim().is_empty() {
            return Order::of(list);
        }
        let mut found: Vec<(i32, usize, u128)> = list.iter().filter_map(|t| crate::search::fuzzy(query, words(t)).map(|f| (f.score, words(t).len(), t.key().raw()))).collect();
        found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        Order::from_keys(found.into_iter().map(|(_, _, k)| k).collect())
    }

    /// Only the items that pass the typed filter `query`, such as `people>100000 country:spain`, in this order; all of them while the query can't be read, since the filter field says what is wrong with it.
    pub fn matching(self, list: &List<T>, query: &str) -> Order<T> {
        match crate::filter::Filter::parse::<T>(query) {
            Ok(filter) if !filter.is_empty() => self.only(list, |t| filter.keeps(t)),
            _ => self,
        }
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
        Rows { list, ranks: self.get().map(|o| &o.ranks), coming: self.coming() }
    }
}

/// The rows a list or a table shows: a list's items in its own order, or in an [`Order`].
pub struct Rows<'a, T: Record> {
    list: &'a List<T>,
    ranks: Option<&'a Arc<Ranks>>,
    coming: bool,
}

impl<'a, T: Record> From<&'a List<T>> for Rows<'a, T> {
    fn from(list: &'a List<T>) -> Self {
        Rows { list, ranks: None, coming: false }
    }
}

impl<'a, T: Record> Rows<'a, T> {
    pub(crate) fn list(&self) -> &'a List<T> {
        self.list
    }

    pub(crate) fn ranks(&self) -> Option<&'a Arc<Ranks>> {
        self.ranks
    }

    /// Whether a new order is being worked out, so what shows may change.
    pub fn coming(&self) -> bool {
        self.coming
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

    pub(crate) fn nest(&self, index: usize) -> Option<Nest> {
        self.ranks?.nest(index)
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

/// A record's fields as a table made from them shows them: each field's name as a head, `people` as "People", with its value, leaving out the record's own key.
pub(crate) fn fields_of<T: Record>(item: &T) -> Vec<(String, Value)> {
    let own = Value::Key(item.key().raw());
    let Value::Map(fields) = item.value() else { return Vec::new() };
    fields.into_iter().filter(|(_, v)| *v != own).map(|(name, v)| (heading(&name), v)).collect()
}

fn heading(name: &str) -> String {
    let mut out = name.replace('_', " ");
    if let Some(first) = out.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    out
}

fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let number = |v: &Value| match v {
        Value::Int(n) => Some(*n as f64),
        Value::UInt(n) => Some(*n as f64),
        Value::Float(n) => Some(*n),
        _ => None,
    };
    match (a, b) {
        (Value::None, Value::None) => Ordering::Equal,
        (Value::None, _) => Ordering::Greater,
        (_, Value::None) => Ordering::Less,
        (Value::Some(a), b) => compare(a, b),
        (a, Value::Some(b)) => compare(a, b),
        (Value::Str(a), Value::Str(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Variant(a, _), Value::Variant(b, _)) => a.cmp(b),
        (Value::List(a), Value::List(b)) => a.len().cmp(&b.len()),
        _ => match (number(a), number(b)) {
            (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        },
    }
}

/// A record's value in one field, as a table made from the record's fields shows it.
pub(crate) struct Plain(pub Value);

impl Cell for Plain {
    fn shown(&self) -> String {
        match &self.0 {
            Value::Str(s) => s.clone(),
            Value::Shared(s) => s.text(),
            Value::Bool(b) => if *b { "Yes" } else { "No" }.to_string(),
            Value::Int(n) => crate::value::grouped(*n),
            Value::UInt(n) => crate::value::grouped(*n),
            Value::Float(n) => n.to_string(),
            Value::Variant(name, _) => name.clone(),
            Value::Some(inner) => Plain((**inner).clone()).shown(),
            Value::List(items) => crate::value::grouped(items.len()),
            _ => String::new(),
        }
    }

    fn numeric(&self) -> bool {
        match &self.0 {
            Value::Some(inner) => Plain((**inner).clone()).numeric(),
            v => matches!(v, Value::Int(_) | Value::UInt(_) | Value::Float(_) | Value::List(_)),
        }
    }

    fn sign(&self) -> i8 {
        match &self.0 {
            Value::Int(n) => n.signum() as i8,
            Value::Float(n) if *n > 0.0 => 1,
            Value::Float(n) if *n < 0.0 => -1,
            Value::UInt(n) if *n > 0 => 1,
            _ => 0,
        }
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
