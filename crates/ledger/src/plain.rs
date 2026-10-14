/// Implements `Field` for a plain value compared with `==`: a register.
#[macro_export]
macro_rules! register {
    ($t:ty) => {
        type Snap = $t;
        fn snap(&self) -> $t {
            ::std::clone::Clone::clone(self)
        }
        fn diff(&self, before: &$t, at: $crate::ErasedPath, out: &mut ::std::vec::Vec<$crate::Op>) {
            if !$crate::Field::same(self, before) {
                out.push($crate::Op::Set { at, before: $crate::Field::value(before), after: $crate::Field::value(self) });
            }
        }
    };
}
use crate::key::Key;
use crate::path::Field;
use crate::value::Value;

/// A field that may sit in the document part of the facts. Everything but `Vec` is; a list in the document is a `List`, so concurrent additions from two devices both survive.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be in the document part of the facts",
    note = "use `List` instead of `Vec`: a list synced as a whole loses one of two concurrent additions; a `Vec` is fine in a `#[session]` field"
)]
pub trait Document: Field {}

macro_rules! ints {
    ($($t:ty => $v:ident as $as:ty),*) => {$(
        impl Field for $t {
            crate::register!($t);
            fn same(&self, other: &Self) -> bool {
                self == other
            }
            fn value(&self) -> Value {
                Value::$v(*self as $as)
            }
            fn from_value(value: &Value) -> Option<Self> {
                match value {
                    Value::Int(i) => (*i).try_into().ok(),
                    Value::UInt(u) => (*u).try_into().ok(),
                    _ => None,
                }
            }
        }
        impl Document for $t {}
    )*};
}

ints!(i8 => Int as i64, i16 => Int as i64, i32 => Int as i64, i64 => Int as i64, isize => Int as i64, u8 => UInt as u64, u16 => UInt as u64, u32 => UInt as u64, u64 => UInt as u64, usize => UInt as u64);

macro_rules! floats {
    ($($t:ty),*) => {$(
        impl Field for $t {
            crate::register!($t);
            fn same(&self, other: &Self) -> bool {
                self.to_bits() == other.to_bits()
            }
            fn value(&self) -> Value {
                Value::Float(*self as f64)
            }
            fn from_value(value: &Value) -> Option<Self> {
                match value {
                    Value::Float(f) => Some(*f as $t),
                    _ => None,
                }
            }
        }
        impl Document for $t {}
    )*};
}

floats!(f32, f64);

impl Field for bool {
    crate::register!(bool);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}
impl Document for bool {}

impl Field for char {
    crate::register!(char);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::UInt(*self as u64)
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::UInt(u) => char::from_u32(*u as u32),
            _ => None,
        }
    }
}
impl Document for char {}

impl Field for String {
    crate::register!(String);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::Str(self.clone())
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        }
    }
}
impl Document for String {}

impl Field for () {
    crate::register!(());
    fn same(&self, _: &Self) -> bool {
        true
    }
    fn value(&self) -> Value {
        Value::Unit
    }
    fn from_value(_: &Value) -> Option<Self> {
        Some(())
    }
}
impl Document for () {}

impl<T: 'static> Field for Key<T> {
    crate::register!(Key<T>);
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn value(&self) -> Value {
        Value::Key(self.raw())
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Key(k) => Some(Key::from_raw(*k)),
            _ => None,
        }
    }
}
impl<T: 'static> Document for Key<T> {}

impl<T: Field> Field for Option<T> {
    crate::register!(Option<T>);
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.same(b),
            (None, None) => true,
            _ => false,
        }
    }
    fn value(&self) -> Value {
        match self {
            Some(v) => Value::Some(Box::new(v.value())),
            None => Value::None,
        }
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::None => Some(None),
            Value::Some(v) => T::from_value(v).map(Some),
            _ => None,
        }
    }
}
impl<T: Document> Document for Option<T> {}

impl<T: Field> Field for Vec<T> {
    crate::register!(Vec<T>);
    fn same(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other).all(|(a, b)| a.same(b))
    }
    fn value(&self) -> Value {
        Value::List(self.iter().map(Field::value).collect())
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::List(items) => items.iter().map(T::from_value).collect(),
            _ => None,
        }
    }
}

impl<A: Field, B: Field> Field for (A, B) {
    crate::register!((A, B));
    fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0) && self.1.same(&other.1)
    }
    fn value(&self) -> Value {
        Value::List(vec![self.0.value(), self.1.value()])
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::List(items) if items.len() == 2 => Some((A::from_value(&items[0])?, B::from_value(&items[1])?)),
            _ => None,
        }
    }
}
impl<A: Document, B: Document> Document for (A, B) {}

impl<T: Field, E: Field> Field for Result<T, E> {
    crate::register!(Result<T, E>);

    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Ok(a), Ok(b)) => a.same(b),
            (Err(a), Err(b)) => a.same(b),
            _ => false,
        }
    }
    fn value(&self) -> Value {
        match self {
            Ok(v) => Value::Variant("ok".into(), Box::new(v.value())),
            Err(e) => Value::Variant("err".into(), Box::new(e.value())),
        }
    }
    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Variant(n, v) if n == "ok" => T::from_value(v).map(Ok),
            Value::Variant(n, v) if n == "err" => E::from_value(v).map(Err),
            _ => None,
        }
    }
}
impl<T: Document, E: Document> Document for Result<T, E> {}
