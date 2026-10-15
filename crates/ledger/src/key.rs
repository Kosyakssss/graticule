use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// The id of a record: the device that made it and a counter. Keys never repeat and never change.
pub struct Key<T> {
    replica: u64,
    n: u64,
    kind: PhantomData<fn() -> T>,
}

impl<T> Key<T> {
    pub const FIXED: u64 = 0;

    pub const fn new(replica: u64, n: u64) -> Self {
        Self { replica, n, kind: PhantomData }
    }

    /// A key for facts built outside `update`, such as named states and samples. No device ever hands these out.
    pub const fn fixed(n: u64) -> Self {
        Self::new(Self::FIXED, n)
    }

    pub const fn replica(self) -> u64 {
        self.replica
    }

    pub const fn counter(self) -> u64 {
        self.n
    }

    pub const fn raw(self) -> u128 {
        (self.replica as u128) << 64 | self.n as u128
    }

    pub const fn from_raw(raw: u128) -> Self {
        Self::new((raw >> 64) as u64, raw as u64)
    }

    pub const fn cast<U>(self) -> Key<U> {
        Key::new(self.replica, self.n)
    }
}

/// The key no record is given, `Key::fixed(0)`: what a reference to a record reads as when facts saved before the reference existed are loaded.
impl<T> Default for Key<T> {
    fn default() -> Self {
        Self::fixed(0)
    }
}

impl<T> Clone for Key<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Key<T> {}

impl<T> PartialEq for Key<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw() == other.raw()
    }
}

impl<T> Eq for Key<T> {}

impl<T> PartialOrd for Key<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Key<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw().cmp(&other.raw())
    }
}

impl<T> Hash for Key<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw().hash(state);
    }
}

impl<T> fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}:{}", self.replica, self.n)
    }
}

impl<T> fmt::Display for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
