use graticule_macros::Facts;

use crate::geometry::Point;

/// Where the view looks on the page: the page point at the view's top-left corner, and how many screen units a page unit takes.
#[derive(Facts, Copy, Debug, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

impl Camera {
    /// The least and most a camera zooms.
    pub const ZOOMS: (f64, f64) = (0.05, 16.0);

    /// The page point under a point of the view.
    pub fn to_page(&self, screen: Point) -> Point {
        Point::new(self.x + screen.x / self.zoom, self.y + screen.y / self.zoom)
    }

    /// Where a page point shows in the view.
    pub fn to_screen(&self, page: Point) -> Point {
        Point::new((page.x - self.x) * self.zoom, (page.y - self.y) * self.zoom)
    }

    /// The camera moved by a distance on screen, as a drag or a wheel pans it.
    pub fn panned(&self, dx: f64, dy: f64) -> Camera {
        Camera { x: self.x - dx / self.zoom, y: self.y - dy / self.zoom, ..*self }
    }

    /// The camera zoomed by `factor` around a point of the view, which stays over the same page point.
    pub fn zoomed(&self, factor: f64, around: Point) -> Camera {
        let zoom = (self.zoom * factor).clamp(Self::ZOOMS.0, Self::ZOOMS.1);
        let page = self.to_page(around);
        Camera { x: page.x - around.x / zoom, y: page.y - around.y / zoom, zoom }
    }
}
