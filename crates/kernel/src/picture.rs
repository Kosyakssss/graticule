use std::hash::{Hash, Hasher};

use crate::ink::Ink;

/// Vector art compiled ahead of time by Engraver from an SVG file: filled shapes in the theme's inks, and a label that
/// screen readers read. Make your own with a build script (see the `engraver` crate), or use one of [`icons`].
#[derive(Debug)]
pub struct Picture {
    pub label: &'static str,
    pub width: f32,
    pub height: f32,
    pub layers: &'static [Layer],
}

/// One filled shape of a picture in one ink, as SVG path data in the picture's own coordinates.
#[derive(Debug)]
pub struct Layer {
    pub ink: Ink,
    pub path: &'static str,
    pub even_odd: bool,
}

impl PartialEq for Picture {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Picture {}

impl Hash for Picture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

/// Graticule's built-in icons, drawn on a 16 by 16 grid in lines of the same weight.
pub mod icons {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
}
