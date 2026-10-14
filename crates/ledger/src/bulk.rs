use std::any::Any;
use std::cell::RefCell;

use crate::path::ErasedPath;

/// The background search that finds which items a bulk change touches: it runs on a snapshot, off the main thread.
pub type Search = Box<dyn FnOnce() -> Vec<u128> + Send>;

type Change = Box<dyn Fn(&mut dyn Any, u128) -> bool>;

/// A change to every matching item of a list too large to change inside one frame. The kernel runs its search as a job, then applies the change a slice per frame, checking each item again as it goes, and keeps the whole of it as one undo step.
pub struct Bulk {
    /// The list it changes.
    pub list: ErasedPath,
    search: Option<Search>,
    change: Change,
}

impl Bulk {
    pub(crate) fn new(list: ErasedPath, search: Search, change: impl Fn(&mut dyn Any, u128) -> bool + 'static) -> Bulk {
        Bulk { list, search: Some(search), change: Box::new(change) }
    }

    /// The search, once: the kernel sends it to a job.
    pub fn search(&mut self) -> Option<Search> {
        self.search.take()
    }

    /// Changes one item found by the search, if it still matches. `list` is the list this bulk change names, reached through the facts.
    pub fn change(&self, list: &mut dyn Any, key: u128) -> bool {
        (self.change)(list, key)
    }
}

thread_local! {
    static DEFERRED: RefCell<Vec<Bulk>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn defer(bulk: Bulk) {
    DEFERRED.with(|d| d.borrow_mut().push(bulk));
}

/// The bulk changes `update` asked for since the last call, oldest first.
pub fn deferred() -> Vec<Bulk> {
    DEFERRED.with(|d| std::mem::take(&mut *d.borrow_mut()))
}
