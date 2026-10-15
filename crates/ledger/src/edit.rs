use std::ops::{Deref, Range};

use crate::outcome::Outcome;
use crate::path::{ErasedPath, FactPath, Field, Step};
use crate::selection::Choose;

use crate::value::Value;

/// A change the kernel asks for on a path into the facts: typing into a field, changing a selection, setting a form's value. It reaches the facts only through the app's `update`, usually as `edit.apply(self)`. It is one pointer wide, so the command enum that carries it stays small.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit(Box<Parts>);

/// What an `Edit` is made of: where, what and which gesture it belongs to, so a drag is one undo step.
#[derive(Clone, Debug, PartialEq)]
pub struct Parts {
    pub path: ErasedPath,
    pub change: Change,
    pub gesture: u64,
}

impl Deref for Edit {
    type Target = Parts;

    fn deref(&self) -> &Parts {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Splice { range: Range<usize>, text: String },
    Select(Choose),
    Set(Value),
    Typed(crate::Typed),
}

impl Edit {
    pub fn new(path: ErasedPath, change: Change, gesture: u64) -> Self {
        Edit(Box::new(Parts { path, change, gesture }))
    }

    /// Whether this edit changes what `path` points at, or something inside it.
    pub fn touches<F, T>(&self, path: FactPath<F, T>) -> bool
    where
        F: Field,
        T: Field,
    {
        self.path.starts_with(&path.erased())
    }

    pub fn is_typing(&self) -> bool {
        matches!(self.change, Change::Splice { .. } | Change::Typed(crate::Typed::Insert(_) | crate::Typed::NewLine | crate::Typed::Delete { .. }))
    }

    /// Makes the change. An app that wants to validate a value or keep an invariant does it next to this call.
    pub fn apply<F: Field>(&self, facts: &mut F) -> Outcome {
        facts.edit(self.path.steps(), &self.change)
    }
}

impl Field for Edit {
    crate::register!(Edit);

    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn value(&self) -> Value {
        let steps = self.path.steps().iter().map(|s| match s {
            Step::Field(i) => Value::UInt(*i as u64),
            Step::Key(k) => Value::Key(*k),
        });
        let change = match &self.change {
            Change::Splice { range, text } => Value::Variant("splice".into(), Box::new(Value::List(vec![Value::UInt(range.start as u64), Value::UInt(range.end as u64), Value::Str(text.clone())]))),
            Change::Select(c) => Value::Variant("select".into(), Box::new(choice_value(c))),
            Change::Set(v) => Value::Variant("set".into(), Box::new(v.clone())),
            Change::Typed(t) => Value::Variant("typed".into(), Box::new(t.value())),
        };
        Value::List(vec![Value::List(steps.collect()), Value::UInt(kind_index(self.path.kind())), change, Value::UInt(self.gesture)])
    }

    fn from_value(value: &Value) -> Option<Self> {
        let Value::List(parts) = value else { return None };
        let [Value::List(steps), Value::UInt(kind), change, Value::UInt(gesture)] = parts.as_slice() else { return None };
        let kind = kind_of(*kind)?;
        let mut path = ErasedPath::ROOT;
        for s in steps {
            path = path.push(
                match s {
                    Value::UInt(i) => Step::Field(*i as u16),
                    Value::Key(k) => Step::Key(*k),
                    _ => return None,
                },
                kind,
            );
        }
        let Value::Variant(name, inner) = change else { return None };
        let change = match (name.as_str(), &**inner) {
            ("splice", Value::List(p)) => match p.as_slice() {
                [Value::UInt(a), Value::UInt(b), Value::Str(t)] => Change::Splice { range: *a as usize..*b as usize, text: t.clone() },
                _ => return None,
            },
            ("select", v) => Change::Select(choice_of(v)?),
            ("set", v) => Change::Set(v.clone()),
            ("typed", v) => Change::Typed(crate::Typed::from_value(v)?),
            _ => return None,
        };
        Some(Edit::new(path, change, *gesture))
    }
}

pub(crate) fn kind_index(kind: crate::Kind) -> u64 {
    match kind {
        crate::Kind::Document => 0,
        crate::Kind::Session => 1,
        crate::Kind::Live => 2,
        crate::Kind::Resource => 3,
    }
}

pub(crate) fn kind_of(i: u64) -> Option<crate::Kind> {
    Some(match i {
        0 => crate::Kind::Document,
        1 => crate::Kind::Session,
        2 => crate::Kind::Live,
        3 => crate::Kind::Resource,
        _ => return None,
    })
}

fn keys(v: &[u128]) -> Value {
    Value::List(v.iter().map(|k| Value::Key(*k)).collect())
}

fn choice_value(c: &Choose) -> Value {
    let (name, inner) = match c {
        Choose::Only(k) => ("only", Value::Key(*k)),
        Choose::Toggle(k) => ("toggle", Value::Key(*k)),
        Choose::Extend { to, between } => ("extend", Value::List(vec![Value::Key(*to), keys(between)])),
        Choose::Focus(k) => ("focus", Value::Key(*k)),
        Choose::All(all) => ("all", keys(all)),
        Choose::Clear => ("clear", Value::Unit),
    };
    Value::Variant(name.into(), Box::new(inner))
}

fn choice_of(v: &Value) -> Option<Choose> {
    let Value::Variant(name, inner) = v else { return None };
    let key_list = |v: &Value| match v {
        Value::List(items) => items
            .iter()
            .map(|i| match i {
                Value::Key(k) => Some(*k),
                _ => None,
            })
            .collect::<Option<Vec<u128>>>(),
        _ => None,
    };
    Some(match (name.as_str(), &**inner) {
        ("only", Value::Key(k)) => Choose::Only(*k),
        ("toggle", Value::Key(k)) => Choose::Toggle(*k),
        ("focus", Value::Key(k)) => Choose::Focus(*k),
        ("extend", Value::List(p)) => match p.as_slice() {
            [Value::Key(to), between] => Choose::Extend { to: *to, between: key_list(between)? },
            _ => return None,
        },
        ("all", v) => Choose::All(key_list(v)?),
        ("clear", _) => Choose::Clear,
        _ => return None,
    })
}
