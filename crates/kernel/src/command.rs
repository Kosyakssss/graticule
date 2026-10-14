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
    pub args: bool,
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

    fn make(entry: usize, subject: Option<u128>) -> Option<Self>;

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
