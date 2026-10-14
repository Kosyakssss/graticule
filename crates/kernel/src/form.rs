//! Settings derived from the app's data: `derive(Form)` describes a struct's labelled fields, `derive(Choice)` an enum's options, and `ui.form` draws a control for each.

use ledger::{ErasedPath, Field, Text, Value};

/// How a setting is changed, chosen by its type.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    /// Yes or no, a switch.
    Switch,
    /// A whole number between `least` and `most`, stepped by one.
    Whole { least: i128, most: i128 },
    /// One line of text, a field.
    Text,
    /// One of a few named values, each with its label.
    Choice(Vec<(&'static str, Value)>),
}

/// A type a setting can have. `derive(Choice)` implements it for an enum of named options.
pub trait Setting: Field {
    fn control() -> Control;
}

impl Setting for bool {
    fn control() -> Control {
        Control::Switch
    }
}

impl Setting for Text {
    fn control() -> Control {
        Control::Text
    }
}

macro_rules! whole {
    ($($t:ty),*) => {$(
        impl Setting for $t {
            fn control() -> Control {
                Control::Whole { least: <$t>::MIN as i128, most: <$t>::MAX as i128 }
            }
        }
    )*};
}

whole!(i8, i16, i32, i64, u8, u16, u32, u64, usize);

/// One setting as a form shows it: its label, where it lives inside the form, what it is now and by default, and how it is changed.
#[derive(Clone, Debug)]
pub struct Item {
    pub label: &'static str,
    pub path: ErasedPath,
    pub value: Value,
    pub default: Value,
    pub control: Control,
}

impl Item {
    /// The value as the form writes it: on or off, the number, the text, or the option's label.
    pub fn shown(&self) -> String {
        shown(&self.control, &self.value)
    }

    pub fn changed(&self) -> bool {
        self.value != self.default
    }
}

pub fn shown(control: &Control, value: &Value) -> String {
    match (control, value) {
        (Control::Switch, Value::Bool(on)) => if *on { "on" } else { "off" }.to_string(),
        (Control::Choice(options), _) => options.iter().find(|(_, v)| v == value).map(|(label, _)| label.to_string()).unwrap_or_default(),
        (_, Value::Int(n)) => crate::value::grouped(*n),
        (_, Value::UInt(n)) => crate::value::grouped(*n),
        (_, Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

/// A group of settings: a struct of facts whose fields marked `#[label("…")]` are each one setting. Made by `derive(Form)`.
pub trait Settings: Field + Default {
    fn items(&self, out: &mut Vec<Item>);
}

/// One step of a whole number, within its type's bounds.
pub fn step(value: &Value, by: i128, least: i128, most: i128) -> Option<Value> {
    let now = match value {
        Value::Int(n) => *n as i128,
        Value::UInt(n) => *n as i128,
        _ => return None,
    };
    let next = now.checked_add(by).filter(|n| (least..=most).contains(n))?;
    Some(match value {
        Value::Int(_) => Value::Int(next as i64),
        _ => Value::UInt(next as u64),
    })
}
