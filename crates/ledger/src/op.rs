use crate::path::ErasedPath;
use crate::value::Value;

/// What a change did to the facts, with enough to reverse it. Operations are what undo reverses, what saving writes and what sync sends.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Insert { list: ErasedPath, after: Option<u128>, key: u128, value: Value },
    Remove { list: ErasedPath, after: Option<u128>, key: u128, value: Value },
    Set { at: ErasedPath, before: Value, after: Value },
    Splice { text: ErasedPath, at: usize, removed: String, inserted: String },
    Select { selection: ErasedPath, before: Value, after: Value },
}

impl Op {
    pub fn path(&self) -> ErasedPath {
        match self {
            Op::Insert { list, .. } | Op::Remove { list, .. } => *list,
            Op::Set { at, .. } => *at,
            Op::Splice { text, .. } => *text,
            Op::Select { selection, .. } => *selection,
        }
    }

    pub fn inverse(&self) -> Op {
        match self.clone() {
            Op::Insert { list, after, key, value } => Op::Remove { list, after, key, value },
            Op::Remove { list, after, key, value } => Op::Insert { list, after, key, value },
            Op::Set { at, before, after } => Op::Set { at, before: after, after: before },
            Op::Splice { text, at, removed, inserted } => Op::Splice { text, at, removed: inserted, inserted: removed },
            Op::Select { selection, before, after } => Op::Select { selection, before: after, after: before },
        }
    }

    pub fn deleted(&self) -> Option<u128> {
        match self {
            Op::Remove { key, .. } => Some(*key),
            _ => None,
        }
    }

    pub fn is_select(&self) -> bool {
        matches!(self, Op::Select { .. })
    }
}
