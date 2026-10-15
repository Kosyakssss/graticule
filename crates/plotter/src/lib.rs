//! PL-01, Plotter: the canvas engine. A [`Board`] is facts: its shapes and the bindings between them are Ledger lists, so every change is an operation that undo reverses, saving keeps and sync carries. Geometry, stacking order, the camera, snapping and export to SVG are pure functions over it, so a board needs no screen. Positions are in page units.

mod board;
mod camera;
mod geometry;
pub mod order;
mod snap;
mod svg;

#[cfg(test)]
mod laws;

pub use board::{Binding, Board, End, Ink, Kind, Shape};
pub use camera::Camera;
pub use geometry::{Bounds, Geometry, Point};
pub use snap::{Guide, Snapped, snap};
pub use svg::svg;
