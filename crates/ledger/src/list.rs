use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use crate::bulk::{self, Bulk};
use crate::journal;
use crate::key::Key;
use crate::op::Op;
use crate::outcome::Outcome;
use crate::path::{ErasedPath, FactPath, Field, Kind, Record, Step};
use crate::plain::Document;
use crate::value::Value;

const CHUNK: usize = 512;
const SEARCH_AT_ONCE: usize = 20_000;
const REMOVE_AT_ONCE: usize = 200;

/// An ordered list of records, found by key. Its storage is shared in chunks, so a copy for a background job costs about the number of chunks, not the number of items, and it keeps a count of each yes-or-no field of its records, so asking whether any item is done is instant at any size.
pub struct List<T: Record> {
    chunks: Vec<Arc<Vec<T>>>,
    ids: Vec<u32>,
    pos: Vec<u32>,
    free: Vec<u32>,
    index: imbl::HashMap<u128, u32>,
    starts: RefCell<Starts>,
    tallies: Vec<u64>,
    len: usize,
    at: ErasedPath,
}

#[derive(Clone, Default)]
struct Starts {
    of: Vec<usize>,
    valid: usize,
}

impl<T: Record> Default for List<T> {
    fn default() -> Self {
        Self {
            chunks: Vec::new(),
            ids: Vec::new(),
            pos: Vec::new(),
            free: Vec::new(),
            index: imbl::HashMap::new(),
            starts: RefCell::default(),
            tallies: vec![0; T::FLAGS],
            len: 0,
            at: ErasedPath::ROOT,
        }
    }
}

impl<T: Record> Clone for List<T> {
    fn clone(&self) -> Self {
        Self {
            chunks: self.chunks.clone(),
            ids: self.ids.clone(),
            pos: self.pos.clone(),
            free: self.free.clone(),
            index: self.index.clone(),
            starts: RefCell::new(self.starts.borrow().clone()),
            tallies: self.tallies.clone(),
            len: self.len,
            at: self.at,
        }
    }
}

impl<T: Record + fmt::Debug> fmt::Debug for List<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Record> List<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Where this list is in the facts, once they have been attached.
    pub fn path(&self) -> ErasedPath {
        self.at
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn has(&self, key: Key<T>) -> bool {
        self.index.contains_key(&key.raw())
    }

    pub fn find(&self, key: Key<T>) -> Option<&T> {
        let (p, i) = self.locate(key)?;
        Some(&self.chunks[p][i])
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + '_ {
        self.chunks.iter().flat_map(|c| c.iter())
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = Key<T>> + '_ {
        self.iter().map(Record::key)
    }

    pub fn first(&self) -> Option<&T> {
        self.chunks.first().and_then(|c| c.first())
    }

    pub fn last(&self) -> Option<&T> {
        self.chunks.last().and_then(|c| c.last())
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let p = self.chunk_of(index);
        let start = self.starts.borrow().of[p];
        self.chunks[p].get(index - start)
    }

    /// Where the record with this key is, counting from the top.
    pub fn position(&self, key: Key<T>) -> Option<usize> {
        let (p, i) = self.locate(key)?;
        self.chunk_of(0);
        let mut starts = self.starts.borrow_mut();
        Self::validate(&mut starts, &self.chunks, p);
        Some(starts.of[p] + i)
    }

    pub fn any(&self, flag: FactPath<T, bool>) -> bool {
        self.count(flag) > 0
    }

    pub fn none(&self, flag: FactPath<T, bool>) -> bool {
        self.count(flag) == 0
    }

    pub fn all(&self, flag: FactPath<T, bool>) -> bool {
        self.count(flag) == self.len
    }

    /// How many records have this yes-or-no field set, kept as the list changes.
    pub fn count(&self, flag: FactPath<T, bool>) -> usize {
        let i = flag.flag().expect("a tally is kept for every yes-or-no field of a record") as usize;
        self.tallies[i] as usize
    }

    pub fn push(&mut self, mut value: T) -> Outcome {
        let key = value.key();
        if self.has(key) {
            return Outcome::Nothing;
        }
        value.attach(self.record_path(key));
        let after = self.last().map(|r| r.key().raw());
        journal::record(|| Op::Insert { list: self.at, after, key: key.raw(), value: value.value() });
        self.insert_at(self.len, value);
        Outcome::Changed
    }

    /// Puts a record right after another one, or first.
    pub fn insert_after(&mut self, after: Option<Key<T>>, mut value: T) -> Outcome {
        let key = value.key();
        if self.has(key) {
            return Outcome::Nothing;
        }
        let at = match after {
            None => 0,
            Some(a) => match self.position(a) {
                Some(p) => p + 1,
                None => return Outcome::Nothing,
            },
        };
        value.attach(self.record_path(key));
        journal::record(|| Op::Insert { list: self.at, after: after.map(Key::raw), key: key.raw(), value: value.value() });
        self.insert_at(at, value);
        Outcome::Changed
    }

    /// Replaces every record with these, as one change that undo takes back whole. Assigning a new list to a field instead isn't recorded, and the kernel says so.
    pub fn replace(&mut self, records: impl IntoIterator<Item = T>) -> Outcome {
        let keys: Vec<Key<T>> = self.keys().collect();
        let mut outcome = Outcome::Nothing;
        for key in keys.into_iter().rev() {
            outcome = outcome.or(self.remove(key));
        }
        for record in records {
            outcome = outcome.or(self.push(record));
        }
        outcome
    }

    pub fn remove(&mut self, key: Key<T>) -> Outcome {
        let Some((p, i)) = self.locate(key) else { return Outcome::Nothing };
        if journal::recording() {
            let after = self.before(p, i).map(|r| r.key().raw());
            let value = self.chunks[p][i].value();
            journal::record(|| Op::Remove { list: self.at, after, key: key.raw(), value });
        }
        self.remove_at(p, i);
        Outcome::Changed
    }

    /// Changes one record in place. The record's key can't change; a list never hands out a record that could escape its record of changes.
    pub fn change(&mut self, key: Key<T>, f: impl FnOnce(&mut T)) -> Outcome {
        let Some((p, i)) = self.locate(key) else { return Outcome::Nothing };
        let path = self.record_path(key);
        let chunk = Arc::make_mut(&mut self.chunks[p]);
        let record = &mut chunk[i];
        record.attach(path);
        let before = record.snap();
        let flags = record.flags();
        f(record);
        assert!(record.key() == key, "a record's key never changes");
        let after = record.flags();
        if journal::recording() {
            let mut ops = Vec::new();
            record.diff(&before, path, &mut ops);
            for op in ops {
                journal::record(|| op);
            }
        }
        self.retally(flags, after);
        Outcome::Changed
    }

    /// Keeps only the records for which `keep` says yes, as one change. On a list too large to change inside one frame, the search runs in the background and the removals a slice per frame, each record checked again as it goes; the app's code is the same either way.
    pub fn retain(&mut self, keep: impl Fn(&T) -> bool + Send + Sync + 'static) -> Outcome
    where
        T: Send + Sync,
    {
        if !journal::recording() {
            if !self.iter().any(|r| !keep(r)) {
                return Outcome::Nothing;
            }
            let kept: Vec<T> = self.iter().filter(|r| keep(r)).cloned().collect();
            let at = self.at;
            *self = kept.into_iter().collect();
            self.at = at;
            return Outcome::Changed;
        }
        let keep = Arc::new(keep);
        let gone = if self.len <= SEARCH_AT_ONCE {
            let gone: Vec<u128> = self.iter().filter(|r| !keep(r)).map(|r| r.key().raw()).collect();
            if gone.is_empty() {
                return Outcome::Nothing;
            }
            if gone.len() <= REMOVE_AT_ONCE {
                for key in gone {
                    let _ = self.remove(Key::from_raw(key));
                }
                return Outcome::Changed;
            }
            Some(gone)
        } else {
            None
        };
        let chunks = self.chunks.clone();
        let check = keep.clone();
        let search: crate::bulk::Search = match gone {
            Some(gone) => Box::new(move || gone),
            None => Box::new(move || chunks.iter().flat_map(|c| c.iter()).filter(|r| !check(r)).map(|r| r.key().raw()).collect()),
        };
        bulk::defer(Bulk::new(self.at, search, move |list, key| {
            let Some(list) = list.downcast_mut::<List<T>>() else { return false };
            let key = Key::from_raw(key);
            list.find(key).is_some_and(|r| !keep(r)) && list.remove(key).changed()
        }));
        Outcome::Changed
    }

    /// Removes every record, as one change; on a large list, a slice per frame like `retain`.
    pub fn clear(&mut self) -> Outcome
    where
        T: Send + Sync,
    {
        if self.is_empty() {
            return Outcome::Nothing;
        }
        self.retain(|_| false)
    }

    fn record_path(&self, key: Key<T>) -> ErasedPath {
        self.at.push(Step::Key(key.raw()), self.at.kind())
    }

    fn locate(&self, key: Key<T>) -> Option<(usize, usize)> {
        let id = *self.index.get(&key.raw())?;
        let p = self.pos[id as usize] as usize;
        let i = self.chunks[p].iter().position(|r| r.key() == key)?;
        Some((p, i))
    }

    fn before(&self, p: usize, i: usize) -> Option<&T> {
        if i > 0 {
            return self.chunks[p].get(i - 1);
        }
        self.chunks[..p].iter().rev().find_map(|c| c.last())
    }

    fn validate(starts: &mut Starts, chunks: &[Arc<Vec<T>>], upto: usize) {
        if starts.of.len() != chunks.len() {
            starts.of.resize(chunks.len(), 0);
            starts.valid = starts.valid.min(chunks.len());
        }
        while starts.valid <= upto && starts.valid < chunks.len() {
            let v = starts.valid;
            starts.of[v] = if v == 0 { 0 } else { starts.of[v - 1] + chunks[v - 1].len() };
            starts.valid += 1;
        }
    }

    fn chunk_of(&self, index: usize) -> usize {
        let mut starts = self.starts.borrow_mut();
        let n = self.chunks.len();
        Self::validate(&mut starts, &self.chunks, n.saturating_sub(1));
        match starts.of.binary_search(&index) {
            Ok(mut p) => {
                while p + 1 < n && starts.of[p + 1] == index {
                    p += 1;
                }
                p
            }
            Err(p) => p.saturating_sub(1),
        }
    }

    fn invalidate(&mut self, from: usize) {
        let starts = self.starts.get_mut();
        starts.valid = starts.valid.min(from);
    }

    fn new_id(&mut self, at: usize) -> u32 {
        let id = self.free.pop().unwrap_or_else(|| {
            self.pos.push(0);
            (self.pos.len() - 1) as u32
        });
        self.ids.insert(at, id);
        for p in at..self.ids.len() {
            self.pos[self.ids[p] as usize] = p as u32;
        }
        id
    }

    fn drop_chunk(&mut self, p: usize) {
        self.chunks.remove(p);
        let id = self.ids.remove(p);
        self.free.push(id);
        self.pos[id as usize] = u32::MAX;
        for q in p..self.ids.len() {
            self.pos[self.ids[q] as usize] = q as u32;
        }
        self.invalidate(p);
    }

    fn insert_at(&mut self, index: usize, value: T) {
        let key = value.key().raw();
        let flags = value.flags();
        if self.chunks.is_empty() {
            self.new_id(0);
            self.chunks.push(Arc::new(Vec::with_capacity(CHUNK)));
        }
        let (p, i) = if index >= self.len {
            let p = self.chunks.len() - 1;
            (p, self.chunks[p].len())
        } else {
            let p = self.chunk_of(index);
            (p, index - self.starts.borrow().of[p])
        };
        let (p, i) = if self.chunks[p].len() >= CHUNK {
            let half = CHUNK / 2;
            let tail: Vec<T> = Arc::make_mut(&mut self.chunks[p]).split_off(half);
            let id = self.new_id(p + 1);
            for r in &tail {
                self.index.insert(r.key().raw(), id);
            }
            self.chunks.insert(p + 1, Arc::new(tail));
            self.invalidate(p);
            if i > half { (p + 1, i - half) } else { (p, i) }
        } else {
            (p, i)
        };
        Arc::make_mut(&mut self.chunks[p]).insert(i, value);
        self.index.insert(key, self.ids[p]);
        self.len += 1;
        self.invalidate(p + 1);
        self.add_tally(flags, 1);
    }

    fn remove_at(&mut self, p: usize, i: usize) {
        let removed = Arc::make_mut(&mut self.chunks[p]).remove(i);
        self.index.remove(&removed.key().raw());
        self.len -= 1;
        self.add_tally(removed.flags(), -1);
        if self.chunks[p].is_empty() {
            self.drop_chunk(p);
        } else {
            self.invalidate(p + 1);
        }
    }

    fn add_tally(&mut self, flags: u64, sign: i64) {
        for (i, t) in self.tallies.iter_mut().enumerate() {
            if flags & (1 << i) != 0 {
                *t = (*t as i64 + sign) as u64;
            }
        }
    }

    fn retally(&mut self, before: u64, after: u64) {
        if before != after {
            self.add_tally(before, -1);
            self.add_tally(after, 1);
        }
    }

    fn apply_here(&mut self, op: &Op) -> bool {
        match op {
            Op::Insert { after, key, value, .. } => {
                let Some(record) = T::from_value(value) else { return false };
                if record.key().raw() != *key || self.has(record.key()) {
                    return false;
                }
                let index = match after {
                    None => 0,
                    Some(a) => match self.position(Key::from_raw(*a)) {
                        Some(p) => p + 1,
                        None => self.len,
                    },
                };
                let mut record = record;
                record.attach(self.record_path(record.key()));
                self.insert_at(index, record);
                true
            }
            Op::Remove { key, .. } => match self.locate(Key::from_raw(*key)) {
                Some((p, i)) => {
                    self.remove_at(p, i);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }
}

impl<T: Record> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut list = List::default();
        for record in iter {
            let _ = list.push(record);
        }
        list
    }
}

impl<'a, T: Record> IntoIterator for &'a List<T> {
    type Item = &'a T;
    type IntoIter = Box<dyn DoubleEndedIterator<Item = &'a T> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl<T: Record> Field for List<T> {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), at: ErasedPath, _: &mut Vec<Op>) {
        if self.at != at {
            journal::replaced(std::any::type_name::<Self>());
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().zip(other.iter()).all(|(a, b)| a.same(b))
    }

    fn value(&self) -> Value {
        Value::List(self.iter().map(Field::value).collect())
    }

    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::List(items) => items.iter().map(T::from_value).collect::<Option<List<T>>>(),
            _ => None,
        }
    }

    fn attach(&mut self, at: ErasedPath) {
        self.at = at;
    }

    fn apply(&mut self, steps: &[Step], op: &Op) -> bool {
        match steps {
            [] => self.apply_here(op),
            [Step::Key(k), rest @ ..] => {
                let key = Key::from_raw(*k);
                let Some((p, i)) = self.locate(key) else { return false };
                let record = &mut Arc::make_mut(&mut self.chunks[p])[i];
                let before = record.flags();
                let applied = record.apply(rest, op);
                let after = record.flags();
                self.retally(before, after);
                applied
            }
            _ => false,
        }
    }

    fn edit(&mut self, steps: &[Step], change: &crate::Change) -> Outcome {
        match steps {
            [Step::Key(k), rest @ ..] => {
                let mut outcome = Outcome::Nothing;
                let _ = self.change(Key::from_raw(*k), |record| outcome = record.edit(rest, change));
                outcome
            }
            _ => Outcome::Nothing,
        }
    }

    fn item_type(&self, steps: &[Step]) -> Option<std::any::TypeId> {
        match steps {
            [] => Some(std::any::TypeId::of::<T>()),
            [Step::Key(k), rest @ ..] => self.find(Key::from_raw(*k))?.item_type(rest),
            _ => None,
        }
    }

    fn reach(&self, steps: &[Step]) -> Option<&dyn Any> {
        match steps {
            [] => Some(self),
            [Step::Key(k), rest @ ..] => self.find(Key::from_raw(*k))?.reach(rest),
            _ => None,
        }
    }

    fn reach_mut(&mut self, steps: &[Step]) -> Option<&mut dyn Any> {
        match steps {
            [] => Some(self),
            [Step::Key(k), rest @ ..] => {
                let (p, i) = self.locate(Key::from_raw(*k))?;
                Arc::make_mut(&mut self.chunks[p])[i].reach_mut(rest)
            }
            _ => None,
        }
    }
}

impl<T: Record> Document for List<T> {}

impl<T: Record> List<T> {
    pub fn kind(&self) -> Kind {
        self.at.kind()
    }
}
