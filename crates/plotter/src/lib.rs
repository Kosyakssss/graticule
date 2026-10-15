//! PL-01, Plotter: the canvas engine. A [`Board`] is facts: its shapes and the bindings between them are Ledger lists, so every change is an operation that undo reverses, saving keeps and sync carries. Geometry, stacking order, snapping, the tools and export to SVG are pure functions over it, so a board needs no screen. The camera is the kernel's, since any canvas can have one. Positions are in page units.

mod board;
pub mod freehand;
mod geometry;
mod index;
pub mod order;
mod snap;
mod svg;
mod tool;

#[cfg(test)]
mod laws;

pub use board::{Binding, Board, End, Ink, Kind, LINE, Shape, text_width};
pub use geometry::{Bounds, Geometry, Point};
pub use index::{Entry, Index};
pub use snap::{Guide, Snapped, snap};
pub use svg::svg;
pub use tool::{Gesture, HANDLES, Intent, Tool};
