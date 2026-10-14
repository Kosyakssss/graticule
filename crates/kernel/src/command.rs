use std::any::TypeId;

use ledger::{Edit, Field};

use crate::message::Message;

/// Which version of a command's label to write: the full one, the short one a control inside a row shows, or the one for several subjects at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Long,
    Short,
    Many(usize),
}

/// One command of the registry, as `derive(Commands)` describes it.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub name: &'static str,
    pub internal: bool,
    pub subject: bool,
    pub subject_type: Option<fn() -> TypeId>,
    pub many: bool,
    /// The arguments the user gives it, in order, besides its subject.
    pub args: &'static [Arg],
    /// What it does, from the `///` comment on its variant, for the help page and what's this.
    pub doc: &'static str,
}

/// One argument a command asks the user for: what it is called, what kind of value it takes, and how to read one from what they typed.
#[derive(Clone, Copy, Debug)]
pub struct Arg {
    pub name: &'static str,
    pub kind: &'static str,
    pub read: fn(&str) -> Result<ledger::Value, Message>,
}

/// A value a command can take as an argument, read from what the user typed in the palette.
pub trait Argument: Field {
    /// What kind of value it is, as the palette says when it asks: "text", "a whole number".
    const KIND: &'static str;

    fn read(text: &str) -> Result<Self, Message>;
}

impl Argument for String {
    const KIND: &'static str = "text";

    fn read(text: &str) -> Result<Self, Message> {
        Ok(text.to_string())
    }
}

impl Argument for bool {
    const KIND: &'static str = "yes or no";

    fn read(text: &str) -> Result<Self, Message> {
        match text.trim().to_lowercase().as_str() {
            "yes" | "y" | "true" | "on" => Ok(true),
            "no" | "n" | "false" | "off" => Ok(false),
            _ => Err(Message::plain(format!("“{text}” isn't yes or no"))),
        }
    }
}

macro_rules! whole {
    ($($t:ty),*) => {$(
        impl Argument for $t {
            const KIND: &'static str = "a whole number";

            fn read(text: &str) -> Result<Self, Message> {
                let digits: String = text.trim().chars().filter(|c| *c != ',' && *c != '_').map(|c| if c == '−' { '-' } else { c }).collect();
                digits.parse().map_err(|_| Message::plain(format!("“{text}” isn't a whole number from {} to {}", crate::value::grouped(<$t>::MIN), crate::value::grouped(<$t>::MAX))))
            }
        }
    )*};
}

whole!(i32, i64, u32, u64, usize);

impl Argument for f64 {
    const KIND: &'static str = "a number";

    fn read(text: &str) -> Result<Self, Message> {
        text.trim().replace(',', "").parse().ok().filter(|n: &f64| n.is_finite()).ok_or_else(|| Message::plain(format!("“{text}” isn't a number")))
    }
}

/// Reads one argument of type `T` into the value a command is rebuilt from. `derive(Commands)` points each `Arg` here.
pub fn read_arg<T: Argument>(text: &str) -> Result<ledger::Value, Message> {
    T::read(text).map(|v| v.value())
}

/// Everything that can happen in an app, made by `derive(Commands)`. `F` is the facts the labels read their subjects from.
pub trait Commands<F>: Field + Sized {
    fn name(&self) -> &'static str;

    fn label(&self, facts: &F, form: Form, out: &mut String);

    fn internal(&self) -> bool;

    fn subject(&self) -> Option<u128>;

    fn with_subject(&self, subject: u128) -> Self;

    fn many(&self) -> bool;

    fn edit(edit: Edit) -> Option<Self>;

    fn as_edit(&self) -> Option<&Edit>;

    fn entries() -> &'static [Entry];

    /// Builds the command an entry names, from its subject and the values of its arguments, or `None` when they don't fit.
    fn make(entry: usize, subject: Option<u128>, args: &[ledger::Value]) -> Option<Self>;

    /// The app's keymap file, `keymap.txt` beside its `Cargo.toml`, built into the app.
    fn keymap() -> &'static str {
        ""
    }

    fn entry(&self) -> Option<usize> {
        let name = self.name();
        Self::entries().iter().position(|e| e.name == name)
    }

    fn label_string(&self, facts: &F, form: Form) -> String {
        let mut out = String::new();
        self.label(facts, form, &mut out);
        out
    }
}

/// A subject that isn't a key: a small value a command acts on, like a market symbol.
pub trait Subject: Copy {
    fn raw(self) -> u128;
    fn from_raw(raw: u128) -> Self;
}

/// Whether a command can run right now, and if not, why. The reason is shown, never hidden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Available {
    Yes,
    No(Message),
}

impl Available {
    pub fn is_yes(&self) -> bool {
        matches!(self, Available::Yes)
    }

    pub fn reason(&self) -> Option<&Message> {
        match self {
            Available::Yes => None,
            Available::No(m) => Some(m),
        }
    }
}

/// `No(reason)` when the condition holds, `Yes` otherwise.
pub fn no_if(condition: bool, reason: Message) -> Available {
    if condition { Available::No(reason) } else { Available::Yes }
}
