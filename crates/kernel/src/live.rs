//! Values that move on their own: live values another thread writes, and derived values worked out in the background.

use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use arc_swap::{ArcSwap, ArcSwapOption};
use ledger::{ErasedPath, Field, Moving, Op, Value};

struct Slot<T> {
    latest: ArcSwap<T>,
    fresh: AtomicBool,
    wake: OnceLock<Arc<dyn Fn() + Send + Sync>>,
}

/// A value written by another thread, an audio callback, a market feed or a download, without locks. The view reads one snapshot per frame: `now` gives the value pinned at the frame's start, so the whole frame sees one value. Mark the field `#[live]`.
pub struct Live<T> {
    slot: Arc<Slot<T>>,
    pinned: Arc<T>,
}

/// The writing end of a live value, for another thread. Publishing wakes the window at most once per frame, however often it publishes.
pub struct Feed<T> {
    slot: Arc<Slot<T>>,
}

impl<T> Clone for Feed<T> {
    fn clone(&self) -> Self {
        Self { slot: self.slot.clone() }
    }
}

impl<T> Feed<T> {
    pub fn publish(&self, value: T) {
        self.slot.latest.store(Arc::new(value));
        if !self.slot.fresh.swap(true, Ordering::AcqRel)
            && let Some(wake) = self.slot.wake.get()
        {
            wake();
        }
    }
}

impl<T> Live<T> {
    pub fn new(value: T) -> Self {
        let pinned = Arc::new(value);
        Self { slot: Arc::new(Slot { latest: ArcSwap::new(pinned.clone()), fresh: AtomicBool::new(false), wake: OnceLock::new() }), pinned }
    }

    /// The writing end, to hand to the thread that produces the values.
    pub fn feed(&self) -> Feed<T> {
        Feed { slot: self.slot.clone() }
    }

    /// The value pinned for this frame.
    pub fn now(&self) -> &T {
        &self.pinned
    }
}

impl<T: Default> Default for Live<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Clone for Live<T> {
    fn clone(&self) -> Self {
        Self { slot: self.slot.clone(), pinned: self.pinned.clone() }
    }
}

impl<T: Send + Sync + 'static> Moving for Live<T> {
    fn feed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(Feed { slot: self.slot.clone() }))
    }

    fn pin(&mut self) -> bool {
        if !self.slot.fresh.swap(false, Ordering::AcqRel) {
            return false;
        }
        let old = std::mem::replace(&mut self.pinned, self.slot.latest.load_full());
        discard(Box::new(old));
        true
    }

    fn wake_with(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        let _ = self.slot.wake.set(wake);
    }
}

impl<T: 'static> Field for Live<T> {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), _: ErasedPath, _: &mut Vec<Op>) {}

    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.slot, &other.slot) && Arc::ptr_eq(&self.pinned, &other.pinned)
    }

    fn value(&self) -> Value {
        Value::Unit
    }

    fn from_value(_: &Value) -> Option<Self> {
        None
    }
}

/// A value worked out in the background from other fields of the facts: `#[derived(Histogram::of, from = [scan])]`. It is worked out again only when one of those fields changes, and the view reads the latest finished value and whether a newer one is coming.
pub struct Derived<T> {
    value: Option<Arc<T>>,
    coming: bool,
    out: Arc<Out<T>>,
}

struct Out<T> {
    value: ArcSwapOption<T>,
    version: Mutex<u64>,
    changed: Condvar,
}

impl<T> Out<T> {
    fn new() -> Self {
        Self { value: ArcSwapOption::empty(), version: Mutex::new(0), changed: Condvar::new() }
    }
}

/// The reading end of a derived value, for another thread: an audio callback that plays what the facts say. It reads the newest finished value without locks, so a real-time thread can read it every time it runs. `Jobs::serve` hands one to a job.
pub struct Watch<T> {
    out: Arc<Out<T>>,
}

impl<T> Clone for Watch<T> {
    fn clone(&self) -> Self {
        Self { out: self.out.clone() }
    }
}

impl<T> Watch<T> {
    /// Runs `f` with the newest finished value, or `None` before the first one, without taking a lock or a reference count.
    pub fn read<R>(&self, f: impl FnOnce(Option<&T>) -> R) -> R {
        let now = self.out.value.load();
        f(now.as_deref())
    }

    /// The newest finished value, kept for as long as it is needed.
    pub fn latest(&self) -> Option<Arc<T>> {
        self.out.value.load_full()
    }

    /// Waits until a value newer than the one numbered `seen` has arrived, or until `most` has passed when it is given, and gives the newest one's number, counting from 1. A thread that waits costs nothing while nothing changes.
    pub fn wait(&self, seen: u64, most: Option<Duration>) -> u64 {
        let version = self.out.version.lock().unwrap_or_else(PoisonError::into_inner);
        let newer = |v: &mut u64| *v <= seen;
        match most {
            Some(most) => *self.out.changed.wait_timeout_while(version, most, newer).unwrap_or_else(PoisonError::into_inner).0,
            None => *self.out.changed.wait_while(version, newer).unwrap_or_else(PoisonError::into_inner),
        }
    }
}

impl<T> Derived<T> {
    /// The latest finished value, or `None` before the first one arrives.
    pub fn get(&self) -> Option<&T> {
        self.value.as_deref()
    }

    /// Whether a newer value is being worked out.
    pub fn coming(&self) -> bool {
        self.coming
    }
}

impl<T> Default for Derived<T> {
    fn default() -> Self {
        Self { value: None, coming: false, out: Arc::new(Out::new()) }
    }
}

impl<T> Clone for Derived<T> {
    fn clone(&self) -> Self {
        Self { value: self.value.clone(), coming: self.coming, out: self.out.clone() }
    }
}

impl<T: Send + Sync + 'static> Moving for Derived<T> {
    fn arrive(&mut self, value: Box<dyn Any + Send>) {
        let Ok(value) = value.downcast::<T>() else { return };
        let value: Arc<T> = Arc::from(value);
        if let Some(old) = self.out.value.swap(Some(value.clone())) {
            discard(Box::new(old));
        }
        *self.out.version.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        self.out.changed.notify_all();
        if let Some(old) = self.value.replace(value) {
            discard(Box::new(old));
        }
    }

    fn watch(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(Watch { out: self.out.clone() }))
    }

    fn coming(&mut self, coming: bool) {
        self.coming = coming;
    }
}

fn discard(old: Box<dyn Send>) {
    static BIN: OnceLock<Option<Sender<Box<dyn Send>>>> = OnceLock::new();
    let bin = BIN.get_or_init(|| {
        let (tx, rx) = channel::<Box<dyn Send>>();
        std::thread::Builder::new().name("graticule-discard".into()).spawn(move || rx.into_iter().for_each(drop)).ok().map(|_| tx)
    });
    if let Some(tx) = bin {
        let _ = tx.send(old);
    }
}

impl<T: 'static> Field for Derived<T> {
    type Snap = ();

    fn snap(&self) {}

    fn diff(&self, _: &(), _: ErasedPath, _: &mut Vec<Op>) {}

    fn same(&self, other: &Self) -> bool {
        match (&self.value, &other.value) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b) && self.coming == other.coming,
            (None, None) => self.coming == other.coming,
            _ => false,
        }
    }

    fn value(&self) -> Value {
        Value::Unit
    }

    fn from_value(_: &Value) -> Option<Self> {
        None
    }
}
