use crate::edit::{kind_index, kind_of};
use crate::path::{ErasedPath, Step};
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

impl ErasedPath {
    /// The path as a value, for the command log and saved operations.
    pub fn to_value(&self) -> Value {
        let steps = self.steps().iter().map(|s| match s {
            Step::Field(i) => Value::UInt(*i as u64),
            Step::Key(k) => Value::Key(*k),
        });
        Value::List(vec![Value::List(steps.collect()), Value::UInt(kind_index(self.kind()))])
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        let Value::List(parts) = value else { return None };
        let [Value::List(steps), Value::UInt(kind)] = parts.as_slice() else { return None };
        let kind = kind_of(*kind)?;
        let mut path = ErasedPath::ROOT;
        for s in steps {
            let step = match s {
                Value::UInt(i) => Step::Field(u16::try_from(*i).ok()?),
                Value::Key(k) => Step::Key(*k),
                _ => return None,
            };
            if path.steps().len() >= crate::DEPTH {
                return None;
            }
            path = path.push(step, kind);
        }
        Some(path)
    }
}

impl Op {
    /// The operation as a value, for saving.
    pub fn to_value(&self) -> Value {
        let after = |a: &Option<u128>| a.map_or(Value::None, Value::Key);
        let (name, parts) = match self {
            Op::Insert { list, after: a, key, value } => ("insert", vec![list.to_value(), after(a), Value::Key(*key), value.clone()]),
            Op::Remove { list, after: a, key, value } => ("remove", vec![list.to_value(), after(a), Value::Key(*key), value.clone()]),
            Op::Set { at, before, after } => ("set", vec![at.to_value(), before.clone(), after.clone()]),
            Op::Splice { text, at, removed, inserted } => ("splice", vec![text.to_value(), Value::UInt(*at as u64), Value::Str(removed.clone()), Value::Str(inserted.clone())]),
            Op::Select { selection, before, after } => ("select", vec![selection.to_value(), before.clone(), after.clone()]),
        };
        Value::Variant(name.into(), Box::new(Value::List(parts)))
    }

    pub fn from_value(value: &Value) -> Option<Op> {
        let Value::Variant(name, inner) = value else { return None };
        let Value::List(parts) = &**inner else { return None };
        let after = |v: &Value| match v {
            Value::None => Some(None),
            Value::Key(k) => Some(Some(*k)),
            _ => None,
        };
        Some(match (name.as_str(), parts.as_slice()) {
            ("insert", [list, a, Value::Key(key), value]) => Op::Insert { list: ErasedPath::from_value(list)?, after: after(a)?, key: *key, value: value.clone() },
            ("remove", [list, a, Value::Key(key), value]) => Op::Remove { list: ErasedPath::from_value(list)?, after: after(a)?, key: *key, value: value.clone() },
            ("set", [at, before, after]) => Op::Set { at: ErasedPath::from_value(at)?, before: before.clone(), after: after.clone() },
            ("splice", [text, Value::UInt(at), Value::Str(removed), Value::Str(inserted)]) => {
                Op::Splice { text: ErasedPath::from_value(text)?, at: *at as usize, removed: removed.clone(), inserted: inserted.clone() }
            }
            ("select", [selection, before, after]) => Op::Select { selection: ErasedPath::from_value(selection)?, before: before.clone(), after: after.clone() },
            _ => return None,
        })
    }
}
