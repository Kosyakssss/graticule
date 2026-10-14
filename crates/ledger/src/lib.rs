//! LD-01, Ledger: the document's types and the command log. Saving, undo, history, replays and sync are built on it. It needs no screen, so any Rust program can keep a ledger.

mod edit;
mod journal;
mod key;
mod list;
mod op;
mod outcome;
mod path;
mod plain;
mod selection;
mod text;
mod value;

pub use edit::{Change, Edit};
pub use journal::{Transaction, record, recording};
pub use key::Key;
pub use list::List;
pub use op::Op;
pub use outcome::Outcome;
pub use path::{DEPTH, ErasedPath, FactPath, Facts, Field, FieldInfo, Find, Kind, Record, Step};
pub use plain::Document;
pub use selection::{Choose, Selection};
pub use text::Text;
pub use value::{Malformed, Value};
