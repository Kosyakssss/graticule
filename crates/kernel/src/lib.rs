//! DK-01, Graticule's kernel: the shell around the operating system, the grid, text, painting and the tree.

pub mod chars;
pub mod fonts;
pub mod gpu;
pub mod grid;
pub mod ink;
pub mod metrics;
pub mod offscreen;
pub mod pace;
pub mod paint;
pub mod px;
pub mod shell;
pub mod text;

#[cfg(test)]
mod laws;
