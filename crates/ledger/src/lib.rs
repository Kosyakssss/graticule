//! LD-01, Ledger: the document's types and the command log. Saving, undo, history, replays and sync are built on it. It needs no screen, so any Rust program can keep a ledger.

mod bulk;
mod edit;
mod history;
mod journal;
mod key;
mod list;
mod op;
mod outcome;
mod path;
mod plain;
mod selection;
mod store;
mod text;
mod typed;
mod value;
pub mod wrap;

pub use bulk::{Bulk, Search, deferred};
pub use edit::{Change, Edit};
pub use history::{History, Kept, Turn};
pub use journal::{Transaction, record, recording, replaced, take_replaced};
pub use key::Key;
pub use list::List;
pub use op::Op;
pub use outcome::Outcome;
pub use path::{DEPTH, Derivation, ErasedPath, FactPath, Facts, Field, FieldInfo, Find, Kind, Moving, Record, Step, Work};
pub use plain::Document;
pub use selection::{Choose, Selection};
pub use store::{Migration, Opened, Stopped, Store, entries, fresh_replica, migrate, note_failures, read, stopped_session};
pub use text::Text;
pub use typed::{Editable, Travel, Typed};
pub use value::{Malformed, Value};
