use std::cell::RefCell;

use crate::op::Op;

thread_local! {
    static OPEN: RefCell<Option<Vec<Op>>> = const { RefCell::new(None) };
}

/// Whether a transaction is open on this thread. Outside one, Ledger's containers change without recording anything: that is how named states, samples and tests build facts.
pub fn recording() -> bool {
    OPEN.with(|open| open.borrow().is_some())
}

pub fn record(op: impl FnOnce() -> Op) {
    OPEN.with(|open| {
        if let Some(ops) = open.borrow_mut().as_mut() {
            ops.push(op());
        }
    });
}

/// The scope that collects a transaction's operations. The kernel opens one around `update`, and Ledger around undo, redo and incoming operations.
pub struct Transaction {
    outer: Option<Vec<Op>>,
}

impl Transaction {
    pub fn begin() -> Transaction {
        let outer = OPEN.with(|open| open.borrow_mut().replace(Vec::new()));
        Transaction { outer }
    }

    pub fn end(self) -> Vec<Op> {
        let ops = OPEN.with(|open| open.borrow_mut().take());
        let outer = self.outer;
        OPEN.with(|open| *open.borrow_mut() = outer);
        ops.unwrap_or_default()
    }
}

thread_local! {
    static REPLACED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

/// Notes that a container was replaced by assigning a new one, which no operation records, so undo couldn't take it back.
pub fn replaced(what: &'static str) {
    REPLACED.with(|r| r.borrow_mut().push(what));
}

/// The containers replaced by assignment since this was last asked, so the kernel can say so.
pub fn take_replaced() -> Vec<&'static str> {
    REPLACED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}
