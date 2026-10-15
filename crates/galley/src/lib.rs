//! GA-01, Galley: the editing core. A [`Doc`] holds text of any length in a rope and is a field of the facts, so every edit is a Ledger operation that undo reverses and saving writes. [`Selections`] are the cursors and selected ranges, as many as the user makes; edits apply at all of them at once, and [`Motion`]s move them. Positions are counted in characters. [`set`] typesets a text in any [`Font`] onto pages, with proper quotes and dashes, [`Hyphenation`] and paragraphs broken as a whole, and [`Typeset::pdf`] writes them at real size. It needs no screen: the editor and any other program build on it.

mod changes;
mod doc;
mod find;
mod fold;
mod font;
mod hyphen;
mod motion;
mod pdf;
mod selection;
mod spelling;
mod typeset;

#[cfg(test)]
mod laws;

pub use changes::Changes;
pub use doc::Doc;
pub use find::{Next, Replacement, Stale};
pub use fold::{Block, Folds};
pub use font::{Font, Segment};
pub use hyphen::Hyphenation;
pub use motion::Motion;
pub use selection::{Selections, Span};
pub use spelling::Spelling;
pub use typeset::{Glyph, Line, Page, Paper, Setting, Typeset, punctuate, set};

pub use ledger::{Mark, Marks};
