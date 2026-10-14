//! DK-01, Graticule's kernel: the shell around the operating system, the grid, text, painting and the tree.

pub mod access;
pub mod app;
pub mod builtin;
pub mod chars;
pub mod command;
pub mod components;
pub mod field;
pub mod fonts;
pub mod form;
pub mod gpu;
pub mod grid;
pub mod hit;
pub mod ink;
pub mod kept;
pub mod keys;
pub mod layout;
pub mod live;
pub mod look;
pub mod menu;
pub mod message;
pub mod metrics;
pub mod offscreen;
pub mod pace;
pub mod paint;
pub mod picture;
pub mod proof;
pub mod px;
mod reload;
pub mod runner;
pub mod scope;
pub mod shell;
pub mod text;
pub mod tree;
pub mod ui;
pub mod value;

pub use accesskit;
pub use app::{App, Cx, Effect, Failure, Jobs, Moment, Path, Reply, Said, States};
pub use command::{Arg, Argument, Available, Commands, Entry, Form, Subject, no_if, read_arg};
pub use ink::Ink;
pub use message::Message;
pub use picture::{Layer, Picture, icons};
pub use tree::{Fill, Span, cols};
pub use form::{Control, Setting, Settings};
pub use live::{Derived, Feed, Live};
pub use ui::{Bind, Ui};
pub use value::{Dates, Integer};

#[cfg(test)]
mod laws;
