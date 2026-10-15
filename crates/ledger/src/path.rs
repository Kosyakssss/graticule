use std::any::Any;
use std::fmt;
use std::marker::PhantomData;

use crate::key::Key;

/// The four kinds of state, marked on each field. They decide where Ledger sends a field's changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Document,
    Session,
    Live,
    Resource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Field(u16),
    Key(u128),
}

pub const DEPTH: usize = 8;

/// A path into the facts without its types: at most eight steps of fields and keys, so it is `Copy` and never allocates.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErasedPath {
    steps: [Step; DEPTH],
    len: u8,
    kind: Kind,
}

impl Default for ErasedPath {
    fn default() -> Self {
        ErasedPath::ROOT
    }
}

impl ErasedPath {
    pub const ROOT: ErasedPath = ErasedPath { steps: [Step::Field(0); DEPTH], len: 0, kind: Kind::Document };

    pub const fn field(index: u16, kind: Kind) -> Self {
        Self::ROOT.push(Step::Field(index), kind)
    }

    pub const fn push(mut self, step: Step, kind: Kind) -> Self {
        assert!((self.len as usize) < DEPTH, "a path is at most eight steps deep");
        if self.len == 0 {
            self.kind = kind;
        }
        self.steps[self.len as usize] = step;
        self.len += 1;
        self
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps[..self.len as usize]
    }

    pub const fn kind(&self) -> Kind {
        self.kind
    }

    pub fn starts_with(&self, prefix: &ErasedPath) -> bool {
        self.steps().starts_with(prefix.steps())
    }

    pub fn first_field(&self) -> Option<u16> {
        match self.steps().first() {
            Some(Step::Field(i)) => Some(*i),
            _ => None,
        }
    }

    pub fn hash64(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for step in self.steps() {
            let (tag, v) = match step {
                Step::Field(i) => (1u128, *i as u128),
                Step::Key(k) => (2, *k),
            };
            for byte in (tag ^ v.rotate_left(7)).to_le_bytes() {
                h = (h ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
        h
    }
}

impl fmt::Debug for ErasedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, step) in self.steps().iter().enumerate() {
            if i > 0 {
                f.write_str("/")?;
            }
            match step {
                Step::Field(n) => write!(f, "{n}")?,
                Step::Key(k) => write!(f, "⟨{:x}:{}⟩", (k >> 64) as u64, *k as u64)?,
            }
        }
        Ok(())
    }
}

/// A typed path from facts `F` to a `T` inside them. `derive(Facts)` makes one per field, named after it: `Todo::draft`.
pub struct FactPath<F, T> {
    path: ErasedPath,
    name: &'static str,
    flag: Option<u8>,
    link: Option<u8>,
    types: PhantomData<fn(&F) -> &T>,
}

impl<F, T> Clone for FactPath<F, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F, T> Copy for FactPath<F, T> {}

impl<F, T> fmt::Debug for FactPath<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.name, self.path)
    }
}

impl<F: Field, T: Field> FactPath<F, T> {
    pub const fn field(index: u16, name: &'static str, kind: Kind, flag: Option<u8>, link: Option<u8>) -> Self {
        Self { path: ErasedPath::field(index, kind), name, flag, link, types: PhantomData }
    }

    pub const fn erased(&self) -> ErasedPath {
        self.path
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn kind(&self) -> Kind {
        self.path.kind()
    }

    pub const fn flag(&self) -> Option<u8> {
        self.flag
    }

    pub const fn link(&self) -> Option<u8> {
        self.link
    }

    pub fn get<'a>(&self, facts: &'a F) -> Option<&'a T> {
        facts.reach(self.path.steps())?.downcast_ref()
    }

    pub fn get_mut<'a>(&self, facts: &'a mut F) -> Option<&'a mut T> {
        facts.reach_mut(self.path.steps())?.downcast_mut()
    }

    pub fn then<U: Field>(self, next: FactPath<T, U>) -> FactPath<F, U> {
        let mut path = self.path;
        for step in next.path.steps() {
            path = path.push(*step, next.kind());
        }
        FactPath { path, name: next.name, flag: next.flag, link: next.link, types: PhantomData }
    }
}

impl<F: Field, T: Field> FactPath<F, crate::List<T>>
where
    T: crate::Record,
{
    /// The record with this key inside a list: `Todo::items.at(key).then(Item::title)`.
    pub fn at(self, key: Key<T>) -> FactPath<F, T> {
        FactPath { path: self.path.push(Step::Key(key.raw()), self.kind()), name: self.name, flag: None, link: None, types: PhantomData }
    }
}

/// Anything that can be a field of the facts. Plain values are registers, compared before and after `update`; Ledger's containers record their own changes as they happen.
#[diagnostic::on_unimplemented(message = "`{Self}` can't be a field of the facts", note = "fields are plain values, Ledger's types (`List`, `Text`, `Selection`) or other `derive(Facts)` types")]
pub trait Field: Clone + 'static {
    type Snap: 'static;

    fn snap(&self) -> Self::Snap;

    fn diff(&self, before: &Self::Snap, at: ErasedPath, out: &mut Vec<crate::Op>);

    fn same(&self, other: &Self) -> bool;

    fn value(&self) -> crate::Value;

    fn from_value(value: &crate::Value) -> Option<Self>;

    fn attach(&mut self, _at: ErasedPath) {}

    fn apply(&mut self, steps: &[Step], op: &crate::Op) -> bool {
        match (steps, op) {
            ([], crate::Op::Set { after, .. }) => match Self::from_value(after) {
                Some(v) => {
                    *self = v;
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    /// Makes an edit the kernel asked for. Plain values take `Set`; `Text` takes splices and `Selection` choices.
    fn edit(&mut self, steps: &[Step], change: &crate::Change) -> crate::Outcome {
        match (steps, change) {
            ([], crate::Change::Set(v)) => match Self::from_value(v) {
                Some(new) if !new.same(self) => {
                    *self = new;
                    crate::Outcome::Changed
                }
                _ => crate::Outcome::Nothing,
            },
            _ => crate::Outcome::Nothing,
        }
    }

    /// Lets go of keys removed from a list: each with the type of record it named, so a selection over another list keeps a key that happens to be equal.
    fn prune(&mut self, _deleted: &[(std::any::TypeId, u128)]) {}

    /// The type of record held by the list at the end of this path, if a list is there.
    fn item_type(&self, _steps: &[Step]) -> Option<std::any::TypeId> {
        None
    }

    /// The raw keys of the records in the list at the end of this path, in order, if a list is there.
    fn item_keys(&self, _steps: &[Step]) -> Option<Vec<u128>> {
        None
    }

    /// The index and kind of the field called `name` in the struct at the end of this path, if a struct is there, so a path can be found from names.
    fn field_named(&self, _steps: &[Step], _name: &str) -> Option<(u16, Kind)> {
        None
    }

    /// Hands each live or derived value directly in this struct to `each`, with its field's index.
    fn moving(&mut self, _each: &mut dyn FnMut(u16, &mut dyn Moving)) {}

    /// The derived values directly in this struct: which field, its name, and the fields it is worked out from.
    fn derivations() -> &'static [Derivation]
    where
        Self: Sized,
    {
        &[]
    }

    /// The work that computes the derived value in `field` from a snapshot of the fields it reads, to run on another thread.
    fn derive_work(&self, _field: u16) -> Option<Work> {
        None
    }

    fn reach(&self, steps: &[Step]) -> Option<&dyn Any> {
        steps.is_empty().then_some(self as &dyn Any)
    }

    fn reach_mut(&mut self, steps: &[Step]) -> Option<&mut dyn Any> {
        if steps.is_empty() { Some(self as &mut dyn Any) } else { None }
    }
}

/// A value that moves on its own: written by another thread (a live value) or worked out in the background (a derived value). The kernel reaches them through `Field::moving`; Ledger only carries them.
pub trait Moving {
    /// Takes the newest value another thread published, if there is one, and says whether it changed.
    fn pin(&mut self) -> bool {
        false
    }

    /// Hands the value a way to wake the window when it publishes.
    fn wake_with(&mut self, _wake: std::sync::Arc<dyn Fn() + Send + Sync>) {}

    /// Takes a finished computation of a derived value.
    fn arrive(&mut self, _value: Box<dyn Any + Send>) {}

    /// Says whether a newer computation is on its way.
    fn coming(&mut self, _coming: bool) {}

    /// The writing end of a live value, for a job that feeds it.
    fn feed(&self) -> Option<Box<dyn Any + Send>> {
        None
    }

    /// The reading end of a derived value, for a job that follows it.
    fn watch(&self) -> Option<Box<dyn Any + Send>> {
        None
    }
}

/// Work that runs on another thread and hands back a value of any type.
pub type Work = Box<dyn FnOnce() -> Box<dyn Any + Send> + Send>;

/// A derived value of a struct of facts: its field, its name and the fields it reads.
#[derive(Clone, Copy, Debug)]
pub struct Derivation {
    pub field: u16,
    pub name: &'static str,
    pub from: &'static [u16],
}

/// Facts: a struct of fields, made by `derive(Facts)`.
pub trait Facts: Field {
    const FIELDS: &'static [FieldInfo];
}

#[derive(Clone, Copy, Debug)]
pub struct FieldInfo {
    pub name: &'static str,
    pub kind: Kind,
}

/// A `Facts` type with a `#[key]` field, so it can live in a `List`. Its yes-or-no fields are counted by every list that holds it, and its fields holding another record's key, or maybe one, are its links, which every list that holds it can follow backwards.
pub trait Record: Facts {
    const FLAGS: usize;
    const LINKS: usize;

    fn key(&self) -> Key<Self>;

    fn flags(&self) -> u64;

    /// The raw key its `i`th link holds, if it holds one.
    fn link(&self, i: usize) -> Option<u128>;
}

/// A field that holds another record's key, or maybe one: a link a list follows backwards.
pub trait Link<U> {
    fn target(&self) -> Option<u128>;
}

impl<U> Link<U> for Key<U> {
    fn target(&self) -> Option<u128> {
        Some(self.raw())
    }
}

impl<U> Link<U> for Option<Key<U>> {
    fn target(&self) -> Option<u128> {
        self.map(|k| k.raw())
    }
}

/// These facts can find a `T` by its key. Generated for every `List<T>` field; labels and `available` use it.
pub trait Find<T> {
    fn find(&self, key: Key<T>) -> Option<&T>;
}
