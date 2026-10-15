use std::ops::{Add, Mul, Sub};

/// A point on the page.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, other: Point) -> f64 {
        (self - other).length()
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        Point::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Point {
    type Output = Point;
    fn mul(self, k: f64) -> Point {
        Point::new(self.x * k, self.y * k)
    }
}

/// A box on the page, its width and height never negative.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Bounds {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Bounds { x: x.min(x + w), y: y.min(y + h), w: w.abs(), h: h.abs() }
    }

    /// The box with two opposite corners at `a` and `b`.
    pub fn around(a: Point, b: Point) -> Self {
        Bounds::new(a.x, a.y, b.x - a.x, b.y - a.y)
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    pub fn intersects(&self, o: &Bounds) -> bool {
        self.x <= o.right() && o.x <= self.right() && self.y <= o.bottom() && o.y <= self.bottom()
    }

    /// Whether `o` lies wholly inside.
    pub fn holds(&self, o: &Bounds) -> bool {
        o.x >= self.x && o.y >= self.y && o.right() <= self.right() && o.bottom() <= self.bottom()
    }

    pub fn union(&self, o: &Bounds) -> Bounds {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        Bounds { x, y, w: self.right().max(o.right()) - x, h: self.bottom().max(o.bottom()) - y }
    }

    pub fn expand(&self, by: f64) -> Bounds {
        Bounds { x: self.x - by, y: self.y - by, w: self.w + 2.0 * by, h: self.h + 2.0 * by }
    }
}

/// The outline of a shape, in page units: what is drawn, hit, snapped to and exported, so they always agree.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Rect(Bounds),
    Ellipse(Bounds),
    /// An open line through its points.
    Line(Vec<Point>),
}

impl Geometry {
    pub fn bounds(&self) -> Bounds {
        match self {
            Geometry::Rect(b) | Geometry::Ellipse(b) => *b,
            Geometry::Line(points) => {
                let first = points.first().copied().unwrap_or_default();
                points.iter().fold(Bounds::around(first, first), |b, &p| b.union(&Bounds::around(p, p)))
            }
        }
    }

    /// How far `p` is from the outline.
    pub fn distance(&self, p: Point) -> f64 {
        p.distance(self.nearest(p))
    }

    /// Whether `p` touches the shape: inside a closed outline, or within `margin` of its line.
    pub fn hit(&self, p: Point, margin: f64) -> bool {
        if !self.bounds().expand(margin).contains(p) {
            return false;
        }
        self.inside(p) || self.distance(p) <= margin
    }

    /// Whether `p` is inside a closed outline. A line has no inside.
    pub fn inside(&self, p: Point) -> bool {
        match self {
            Geometry::Rect(b) => b.contains(p),
            Geometry::Ellipse(b) => {
                let (c, rx, ry) = (b.center(), b.w / 2.0, b.h / 2.0);
                if rx <= 0.0 || ry <= 0.0 {
                    return false;
                }
                let (dx, dy) = ((p.x - c.x) / rx, (p.y - c.y) / ry);
                dx * dx + dy * dy <= 1.0
            }
            Geometry::Line(_) => false,
        }
    }

    /// The point on the outline nearest to `p`.
    pub fn nearest(&self, p: Point) -> Point {
        match self {
            Geometry::Line(points) => nearest_on_path(points, p),
            _ => nearest_on_path(&self.outline(), p),
        }
    }

    /// Where the line from `from` toward `to` first crosses the outline, if it does before reaching `to`.
    pub fn crossing(&self, from: Point, to: Point) -> Option<Point> {
        let points = match self {
            Geometry::Line(points) => points.clone(),
            _ => self.outline(),
        };
        points.windows(2).filter_map(|w| cross(from, to, w[0], w[1])).min_by(|a, b| from.distance(*a).total_cmp(&from.distance(*b)))
    }

    /// A closed outline as points, the first repeated at the end; an ellipse is drawn through 64 of them.
    fn outline(&self) -> Vec<Point> {
        match self {
            Geometry::Rect(b) => vec![Point::new(b.x, b.y), Point::new(b.right(), b.y), Point::new(b.right(), b.bottom()), Point::new(b.x, b.bottom()), Point::new(b.x, b.y)],
            Geometry::Ellipse(b) => {
                let c = b.center();
                (0..=64)
                    .map(|i| {
                        let t = f64::from(i) / 64.0 * std::f64::consts::TAU;
                        Point::new(c.x + b.w / 2.0 * t.cos(), c.y + b.h / 2.0 * t.sin())
                    })
                    .collect()
            }
            Geometry::Line(points) => points.clone(),
        }
    }
}

fn nearest_on_path(points: &[Point], p: Point) -> Point {
    let Some(&first) = points.first() else { return p };
    points.windows(2).map(|w| nearest_on_segment(w[0], w[1], p)).fold(first, |best, q| if q.distance(p) < best.distance(p) { q } else { best })
}

fn nearest_on_segment(a: Point, b: Point, p: Point) -> Point {
    let d = b - a;
    let len = d.x * d.x + d.y * d.y;
    if len == 0.0 {
        return a;
    }
    let t = (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len).clamp(0.0, 1.0);
    a + d * t
}

fn cross(p: Point, p2: Point, q: Point, q2: Point) -> Option<Point> {
    let (r, s) = (p2 - p, q2 - q);
    let denom = r.x * s.y - r.y * s.x;
    if denom == 0.0 {
        return None;
    }
    let qp = q - p;
    let t = (qp.x * s.y - qp.y * s.x) / denom;
    let u = (qp.x * r.y - qp.y * r.x) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| p + r * t)
}
