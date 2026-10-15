//! GA-01, Galley: the editing core. A [`Doc`] holds text of any length in a rope and is a field of the facts, so every edit is a Ledger operation that undo reverses and saving writes. [`Selections`] are the cursors and selected ranges, as many as the user makes; edits apply at all of them at once, and [`Motion`]s move them. Positions are counted in characters. It needs no screen: the editor and any other program build on it.

mod changes;
mod doc;
mod find;
mod motion;
mod selection;

#[cfg(test)]
mod laws;

pub use changes::Changes;
pub use doc::Doc;
pub use motion::Motion;
pub use selection::{Selections, Span};
