//! DK-01, Graticule's kernel: the shell around the operating system, the grid, text, painting and the tree.

pub mod access;
pub mod app;
pub mod builtin;
pub mod chars;
pub mod command;
pub mod components;
pub mod field;
pub mod fonts;
pub mod gpu;
pub mod grid;
pub mod hit;
pub mod ink;
pub mod kept;
pub mod keys;
pub mod layout;
pub mod message;
pub mod metrics;
pub mod offscreen;
pub mod pace;
pub mod paint;
pub mod px;
pub mod runner;
pub mod shell;
pub mod text;
pub mod tree;
pub mod ui;

pub use app::{App, Cx, Effect, Failure, Moment, Path, Said, States};
pub use command::{Available, Commands, Entry, Form, Subject, no_if};
pub use message::Message;
pub use tree::{Fill, Span, cols};
pub use ui::{Bind, Ui};

#[cfg(test)]
mod laws;
